use super::models::*;

/// Apply a route policy to a BGP route. Returns the terminal action.
#[allow(dead_code)]
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

/// Parse a simple policy DSL string into a RoutePolicy.
///
/// Format:
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
pub fn parse_policy(input: &str) -> Result<RoutePolicy, String> {
    let input = input.trim();
    let lines: Vec<&str> = input.lines().map(|l| l.trim()).collect();

    // Parse header: policy "name" {
    let first = lines.first().ok_or("Empty policy")?;
    let name = extract_quoted(first).ok_or("Expected policy \"name\" {")?;

    let mut terms = Vec::new();
    let mut default_action = PolicyAction::Accept;
    let mut i = 1;

    while i < lines.len() {
        let line = lines[i];

        if line.starts_with("term ") {
            let (term, consumed) = parse_term(&lines[i..])?;
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

fn parse_term(lines: &[&str]) -> Result<(PolicyTerm, usize), String> {
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

fn extract_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_policy() {
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
}
