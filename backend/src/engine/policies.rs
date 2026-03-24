use super::models::*;

/// Apply a route policy to a BGP route. Returns the terminal action.
pub fn apply_policy(policy: &RoutePolicy, route: &mut BgpRoute) -> PolicyAction {
    for term in &policy.terms {
        if matches_conditions(&term.match_conditions, route) {
            // Apply set actions
            for action in &term.actions {
                apply_set_action(action, route);
            }
            return term.terminal_action.clone();
        }
    }
    policy.default_action.clone()
}

fn matches_conditions(conditions: &MatchConditions, route: &BgpRoute) -> bool {
    // If no conditions specified, match everything
    let mut has_condition = false;

    if let Some(ref prefixes) = conditions.prefix_list {
        has_condition = true;
        if !prefixes.iter().any(|p| route.prefix == *p) {
            return false;
        }
    }

    if let Some(ref community) = conditions.community {
        has_condition = true;
        if !route.attributes.communities.contains(community) {
            return false;
        }
    }

    if let Some(ref as_path_regex) = conditions.as_path_regex {
        has_condition = true;
        let as_path_str: String = route
            .attributes
            .as_path
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        // Simple prefix/suffix matching (not full regex for simplicity)
        if as_path_regex.starts_with('^') {
            let pattern = &as_path_regex[1..].replace('_', " ");
            if !as_path_str.starts_with(pattern) {
                return false;
            }
        } else if as_path_regex.ends_with('$') {
            let pattern = &as_path_regex[..as_path_regex.len() - 1].replace('_', " ");
            if !as_path_str.ends_with(pattern) {
                return false;
            }
        } else {
            let pattern = as_path_regex.replace('_', " ");
            if !as_path_str.contains(&pattern) {
                return false;
            }
        }
    }

    // If no conditions were specified, it matches everything
    has_condition || true
}

fn apply_set_action(action: &PolicySetAction, route: &mut BgpRoute) {
    match action {
        PolicySetAction::SetLocalPref(val) => {
            route.attributes.local_pref = *val;
        }
        PolicySetAction::SetMed(val) => {
            route.attributes.med = *val;
        }
        PolicySetAction::PrependAsPath { asn, count } => {
            for _ in 0..*count {
                route.attributes.as_path.insert(0, *asn);
            }
        }
        PolicySetAction::AddCommunity(comm) => {
            if !route.attributes.communities.contains(comm) {
                route.attributes.communities.push(comm.clone());
            }
        }
        PolicySetAction::RemoveCommunity(comm) => {
            route.attributes.communities.retain(|c| c != comm);
        }
    }
}

/// Parse a policy from either the simple DSL or Juniper-style syntax.
/// Auto-detects format based on content.
///
/// ## Simple DSL format:
/// ```text
/// policy "name" {
///   term 1 {
///     match community "65001:100"
///     match as-path "^65002_"
///     match prefix "10.0.0.0/8"
///     set local-pref 150
///     set med 50
///     prepend-as 65001 3
///     add-community "no-export"
///     accept
///   }
///   default reject
/// }
/// ```
///
/// ## Juniper-style format:
/// ```text
/// policy-options {
///   policy-statement prefer-customer {
///     term 1 {
///       from {
///         community 65001:100;
///         as-path "^65002";
///         route-filter 10.0.0.0/8 exact;
///       }
///       then {
///         local-preference 150;
///         metric 50;
///         as-path-prepend 65001 3;
///         community add no-export;
///         accept;
///       }
///     }
///     then reject;
///   }
/// }
/// ```
///
/// The Juniper parser also accepts `policy-statement` without the
/// outer `policy-options` wrapper.
pub fn parse_policy(input: &str) -> Result<RoutePolicy, String> {
    let trimmed = input.trim();

    // Auto-detect format
    if trimmed.starts_with("policy-options") || trimmed.starts_with("policy-statement") {
        parse_juniper_policy(trimmed)
    } else {
        parse_simple_policy(trimmed)
    }
}

// ── Simple DSL parser ──

fn parse_simple_policy(input: &str) -> Result<RoutePolicy, String> {
    let lines: Vec<&str> = input.lines().map(|l| l.trim()).collect();

    let first = lines.first().ok_or("Empty policy")?;
    let name = extract_quoted(first).ok_or("Expected policy \"name\" {")?;

    let mut terms = Vec::new();
    let mut default_action = PolicyAction::Accept;
    let mut i = 1;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("term ") {
            let (term, consumed) = parse_simple_term(&lines[i..])?;
            terms.push(term);
            i += consumed;
        } else if line.starts_with("default ") {
            let action_str = line.strip_prefix("default ").unwrap().trim();
            default_action = match action_str {
                "accept" => PolicyAction::Accept,
                "reject" => PolicyAction::Reject,
                _ => return Err(format!("Unknown default action: {}", action_str)),
            };
            i += 1;
        } else {
            i += 1;
        }
    }

    Ok(RoutePolicy {
        name,
        terms,
        default_action,
    })
}

fn parse_simple_term(lines: &[&str]) -> Result<(PolicyTerm, usize), String> {
    let header = lines[0];
    let term_name = header
        .strip_prefix("term ")
        .and_then(|s| s.strip_suffix(" {").or(s.strip_suffix('{')))
        .unwrap_or("unnamed")
        .trim()
        .to_string();

    let mut match_conditions = MatchConditions::default();
    let mut actions = Vec::new();
    let mut terminal_action = PolicyAction::Accept;
    let mut i = 1;

    while i < lines.len() {
        let line = lines[i];
        if line == "}" {
            i += 1;
            break;
        }

        if line.starts_with("match community ") {
            let val = extract_quoted(line).unwrap_or_default();
            match_conditions.community = Some(val);
        } else if line.starts_with("match as-path ") {
            let val = extract_quoted(line).unwrap_or_default();
            match_conditions.as_path_regex = Some(val);
        } else if line.starts_with("match prefix ") {
            let val = extract_quoted(line).unwrap_or_default();
            let prefixes = match_conditions.prefix_list.get_or_insert_with(Vec::new);
            prefixes.push(val);
        } else if line.starts_with("set local-pref ") {
            let val: u32 = line
                .strip_prefix("set local-pref ")
                .unwrap()
                .parse()
                .map_err(|_| "Invalid local-pref value")?;
            actions.push(PolicySetAction::SetLocalPref(val));
        } else if line.starts_with("set med ") {
            let val: u32 = line
                .strip_prefix("set med ")
                .unwrap()
                .parse()
                .map_err(|_| "Invalid med value")?;
            actions.push(PolicySetAction::SetMed(val));
        } else if line.starts_with("prepend-as ") {
            let parts: Vec<&str> = line.strip_prefix("prepend-as ").unwrap().split_whitespace().collect();
            if parts.len() != 2 {
                return Err("prepend-as requires ASN and count".to_string());
            }
            let asn: u32 = parts[0].parse().map_err(|_| "Invalid ASN")?;
            let count: u32 = parts[1].parse().map_err(|_| "Invalid count")?;
            actions.push(PolicySetAction::PrependAsPath { asn, count });
        } else if line.starts_with("add-community ") {
            let val = extract_quoted(line).unwrap_or_default();
            actions.push(PolicySetAction::AddCommunity(val));
        } else if line.starts_with("remove-community ") {
            let val = extract_quoted(line).unwrap_or_default();
            actions.push(PolicySetAction::RemoveCommunity(val));
        } else if line == "accept" {
            terminal_action = PolicyAction::Accept;
        } else if line == "reject" {
            terminal_action = PolicyAction::Reject;
        }

        i += 1;
    }

    Ok((
        PolicyTerm {
            name: term_name,
            match_conditions,
            actions,
            terminal_action,
        },
        i,
    ))
}

// ── Juniper-style parser ──

fn parse_juniper_policy(input: &str) -> Result<RoutePolicy, String> {
    let lines: Vec<&str> = input.lines().map(|l| l.trim()).collect();
    let mut i = 0;

    // Skip outer `policy-options {` wrapper if present
    if lines.get(i).map_or(false, |l| l.starts_with("policy-options")) {
        i += 1; // skip "policy-options {"
    }

    // Skip blank/brace lines to find `policy-statement`
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("policy-statement") {
            break;
        }
        i += 1;
    }

    if i >= lines.len() {
        return Err("Expected policy-statement".to_string());
    }

    // Parse: policy-statement <name> {
    let ps_line = lines[i];
    let name = ps_line
        .strip_prefix("policy-statement")
        .and_then(|s| {
            let s = s.trim();
            s.strip_suffix('{').map(|s| s.trim().to_string())
        })
        .ok_or("Expected: policy-statement <name> {")?;

    if name.is_empty() {
        return Err("Policy statement name cannot be empty".to_string());
    }

    i += 1;

    let mut terms = Vec::new();
    let mut default_action = PolicyAction::Accept;

    while i < lines.len() {
        let line = lines[i];

        if line == "}" {
            break;
        }

        if line.starts_with("term ") {
            let (term, consumed) = parse_juniper_term(&lines[i..])?;
            terms.push(term);
            i += consumed;
        } else if line.starts_with("then ") && !line.contains('{') {
            // Default action: `then reject;` or `then accept;`
            let action_str = line
                .strip_prefix("then ")
                .unwrap()
                .trim()
                .trim_end_matches(';');
            default_action = parse_action_keyword(action_str)?;
            i += 1;
        } else {
            i += 1;
        }
    }

    Ok(RoutePolicy {
        name,
        terms,
        default_action,
    })
}

fn parse_juniper_term(lines: &[&str]) -> Result<(PolicyTerm, usize), String> {
    let header = lines[0];
    let term_name = header
        .strip_prefix("term ")
        .and_then(|s| s.strip_suffix('{').or(s.strip_suffix(" {")))
        .unwrap_or("unnamed")
        .trim()
        .to_string();

    let mut match_conditions = MatchConditions::default();
    let mut actions = Vec::new();
    let mut terminal_action = PolicyAction::Accept;
    let mut i = 1;

    while i < lines.len() {
        let line = lines[i];

        if line == "}" {
            i += 1;
            break;
        }

        if line.starts_with("from") && (line.ends_with('{') || line == "from {") {
            // Parse from { ... } block
            i += 1;
            while i < lines.len() && lines[i] != "}" {
                parse_juniper_from_line(lines[i], &mut match_conditions)?;
                i += 1;
            }
            i += 1; // skip closing }
        } else if line.starts_with("then") && line.ends_with('{') {
            // Parse then { ... } block
            i += 1;
            while i < lines.len() && lines[i] != "}" {
                parse_juniper_then_line(lines[i], &mut actions, &mut terminal_action)?;
                i += 1;
            }
            i += 1; // skip closing }
        } else if line.starts_with("then ") && !line.contains('{') {
            // Inline then: `then accept;`
            let action_str = line
                .strip_prefix("then ")
                .unwrap()
                .trim()
                .trim_end_matches(';');
            terminal_action = parse_action_keyword(action_str)?;
            i += 1;
        } else {
            i += 1;
        }
    }

    Ok((
        PolicyTerm {
            name: term_name,
            match_conditions,
            actions,
            terminal_action,
        },
        i,
    ))
}

fn parse_juniper_from_line(line: &str, conditions: &mut MatchConditions) -> Result<(), String> {
    let line = line.trim().trim_end_matches(';');

    if line.starts_with("community ") {
        let val = line.strip_prefix("community ").unwrap().trim();
        // Support both quoted and unquoted values
        let val = val.trim_matches('"').to_string();
        conditions.community = Some(val);
    } else if line.starts_with("as-path ") {
        let val = line.strip_prefix("as-path ").unwrap().trim();
        let val = val.trim_matches('"').to_string();
        conditions.as_path_regex = Some(val);
    } else if line.starts_with("route-filter ") {
        // route-filter 10.0.0.0/8 exact;
        let rest = line.strip_prefix("route-filter ").unwrap().trim();
        let prefix = rest.split_whitespace().next().unwrap_or("").to_string();
        if !prefix.is_empty() {
            let prefixes = conditions.prefix_list.get_or_insert_with(Vec::new);
            prefixes.push(prefix);
        }
    } else if line.starts_with("prefix-list ") || line.starts_with("route-filter-list ") {
        // Treat as a single prefix match
        let rest = if line.starts_with("prefix-list ") {
            line.strip_prefix("prefix-list ").unwrap()
        } else {
            line.strip_prefix("route-filter-list ").unwrap()
        };
        let prefix = rest.trim().trim_matches('"').to_string();
        if !prefix.is_empty() {
            let prefixes = conditions.prefix_list.get_or_insert_with(Vec::new);
            prefixes.push(prefix);
        }
    }

    Ok(())
}

fn parse_juniper_then_line(
    line: &str,
    actions: &mut Vec<PolicySetAction>,
    terminal_action: &mut PolicyAction,
) -> Result<(), String> {
    let line = line.trim().trim_end_matches(';');

    if line.starts_with("local-preference ") {
        let val: u32 = line
            .strip_prefix("local-preference ")
            .unwrap()
            .trim()
            .parse()
            .map_err(|_| "Invalid local-preference value")?;
        actions.push(PolicySetAction::SetLocalPref(val));
    } else if line.starts_with("metric ") {
        let val: u32 = line
            .strip_prefix("metric ")
            .unwrap()
            .trim()
            .parse()
            .map_err(|_| "Invalid metric value")?;
        actions.push(PolicySetAction::SetMed(val));
    } else if line.starts_with("as-path-prepend ") {
        // as-path-prepend "65001 65001 65001" or as-path-prepend 65001 3
        let rest = line.strip_prefix("as-path-prepend ").unwrap().trim();
        let rest = rest.trim_matches('"');

        // Check if it's "ASN count" format
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() == 2 {
            if let (Ok(asn), Ok(count)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                // Could be "ASN count" or "ASN ASN" — if both are the same, treat as repeated
                if asn == count {
                    // Ambiguous, treat as two repetitions of the same ASN
                    actions.push(PolicySetAction::PrependAsPath { asn, count: 2 });
                } else if count <= 10 {
                    // Likely "ASN count" format
                    actions.push(PolicySetAction::PrependAsPath { asn, count });
                } else {
                    // Two different ASNs repeated once each
                    actions.push(PolicySetAction::PrependAsPath { asn, count: 1 });
                    actions.push(PolicySetAction::PrependAsPath {
                        asn: count,
                        count: 1,
                    });
                }
                return Ok(());
            }
        }

        // Repeated ASN format: "65001 65001 65001"
        if !parts.is_empty() {
            if let Ok(asn) = parts[0].parse::<u32>() {
                let count = parts.iter().filter(|&&p| p.parse::<u32>().ok() == Some(asn)).count() as u32;
                actions.push(PolicySetAction::PrependAsPath { asn, count });
            }
        }
    } else if line.starts_with("community add ") {
        let val = line
            .strip_prefix("community add ")
            .unwrap()
            .trim()
            .trim_matches('"')
            .to_string();
        actions.push(PolicySetAction::AddCommunity(val));
    } else if line.starts_with("community delete ") || line.starts_with("community remove ") {
        let prefix = if line.starts_with("community delete ") {
            "community delete "
        } else {
            "community remove "
        };
        let val = line
            .strip_prefix(prefix)
            .unwrap()
            .trim()
            .trim_matches('"')
            .to_string();
        actions.push(PolicySetAction::RemoveCommunity(val));
    } else if line.starts_with("community set ") {
        // community set replaces — we model as add since we don't have a "replace" action
        let val = line
            .strip_prefix("community set ")
            .unwrap()
            .trim()
            .trim_matches('"')
            .to_string();
        actions.push(PolicySetAction::AddCommunity(val));
    } else if line == "accept" {
        *terminal_action = PolicyAction::Accept;
    } else if line == "reject" {
        *terminal_action = PolicyAction::Reject;
    } else if line == "next policy" || line == "next-policy" {
        // Juniper "next policy" — we treat as accept (continue evaluation)
        *terminal_action = PolicyAction::Accept;
    }

    Ok(())
}

fn parse_action_keyword(s: &str) -> Result<PolicyAction, String> {
    match s.trim() {
        "accept" => Ok(PolicyAction::Accept),
        "reject" => Ok(PolicyAction::Reject),
        other => Err(format!("Unknown action: {}", other)),
    }
}

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_policy() {
        let input = r#"
policy "test-policy" {
  term 1 {
    match community "65001:100"
    set local-pref 150
    accept
  }
  default reject
}
"#;
        let policy = parse_policy(input).unwrap();
        assert_eq!(policy.name, "test-policy");
        assert_eq!(policy.terms.len(), 1);
        assert_eq!(policy.default_action, PolicyAction::Reject);
        assert_eq!(
            policy.terms[0].match_conditions.community,
            Some("65001:100".to_string())
        );
    }

    #[test]
    fn test_parse_juniper_policy_with_wrapper() {
        let input = r#"
policy-options {
  policy-statement prefer-customer {
    term set-pref {
      from {
        community 65001:100;
        as-path "^65002";
      }
      then {
        local-preference 150;
        accept;
      }
    }
    then reject;
  }
}
"#;
        let policy = parse_policy(input).unwrap();
        assert_eq!(policy.name, "prefer-customer");
        assert_eq!(policy.terms.len(), 1);
        assert_eq!(policy.terms[0].name, "set-pref");
        assert_eq!(policy.default_action, PolicyAction::Reject);
        assert_eq!(
            policy.terms[0].match_conditions.community,
            Some("65001:100".to_string())
        );
        assert_eq!(
            policy.terms[0].match_conditions.as_path_regex,
            Some("^65002".to_string())
        );
        assert!(matches!(
            policy.terms[0].actions[0],
            PolicySetAction::SetLocalPref(150)
        ));
    }

    #[test]
    fn test_parse_juniper_policy_without_wrapper() {
        let input = r#"
policy-statement block-bogons {
  term deny-rfc1918 {
    from {
      route-filter 10.0.0.0/8 exact;
      route-filter 172.16.0.0/12 exact;
    }
    then reject;
  }
  then accept;
}
"#;
        let policy = parse_policy(input).unwrap();
        assert_eq!(policy.name, "block-bogons");
        assert_eq!(policy.terms.len(), 1);
        assert_eq!(policy.terms[0].terminal_action, PolicyAction::Reject);
        assert_eq!(policy.default_action, PolicyAction::Accept);
        let prefixes = policy.terms[0]
            .match_conditions
            .prefix_list
            .as_ref()
            .unwrap();
        assert_eq!(prefixes.len(), 2);
        assert_eq!(prefixes[0], "10.0.0.0/8");
        assert_eq!(prefixes[1], "172.16.0.0/12");
    }

    #[test]
    fn test_parse_juniper_community_and_metric() {
        let input = r#"
policy-statement set-med {
  term adjust {
    from {
      community no-export;
    }
    then {
      metric 100;
      community add 65001:200;
      community delete 65001:100;
      accept;
    }
  }
  then reject;
}
"#;
        let policy = parse_policy(input).unwrap();
        assert_eq!(policy.name, "set-med");
        let term = &policy.terms[0];
        assert_eq!(
            term.match_conditions.community,
            Some("no-export".to_string())
        );
        assert_eq!(term.actions.len(), 3);
        assert!(matches!(term.actions[0], PolicySetAction::SetMed(100)));
        assert!(matches!(
            &term.actions[1],
            PolicySetAction::AddCommunity(c) if c == "65001:200"
        ));
        assert!(matches!(
            &term.actions[2],
            PolicySetAction::RemoveCommunity(c) if c == "65001:100"
        ));
    }

    #[test]
    fn test_parse_juniper_as_path_prepend() {
        let input = r#"
policy-statement prepend-path {
  term prepend {
    then {
      as-path-prepend "65001 65001 65001";
      accept;
    }
  }
  then accept;
}
"#;
        let policy = parse_policy(input).unwrap();
        let term = &policy.terms[0];
        assert_eq!(term.actions.len(), 1);
        match &term.actions[0] {
            PolicySetAction::PrependAsPath { asn, count } => {
                assert_eq!(*asn, 65001);
                assert_eq!(*count, 3);
            }
            _ => panic!("Expected PrependAsPath"),
        }
    }
}
