const BASE = "/api/v1";

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...options,
  });
  if (!res.ok) {
    const err = await res.json().catch(() => ({ message: res.statusText }));
    throw new Error(err.message || res.statusText);
  }
  return res.json();
}

// Topology
export const listAS = () => request<any[]>("/topology/as");
export const createAS = (asn: number, name: string) =>
  request<{ id: string }>("/topology/as", {
    method: "POST",
    body: JSON.stringify({ asn, name }),
  });
export const deleteAS = (id: string) =>
  request(`/topology/as/${id}`, { method: "DELETE" });
export const listRouters = (asId: string) =>
  request<any[]>(`/topology/as/${asId}/routers`);
export const createRouter = (
  asId: string,
  name: string,
  routerIdIp: string,
  x: number,
  y: number,
) =>
  request<{ id: string }>(`/topology/as/${asId}/routers`, {
    method: "POST",
    body: JSON.stringify({
      name,
      router_id_ip: routerIdIp,
      position_x: x,
      position_y: y,
    }),
  });
export const deleteRouter = (id: string) =>
  request(`/topology/routers/${id}`, { method: "DELETE" });
export const addInterface = (
  routerId: string,
  name: string,
  ipAddress: string,
  bandwidth: number,
  cost: number,
) =>
  request<{ id: string }>(`/topology/routers/${routerId}/interfaces`, {
    method: "POST",
    body: JSON.stringify({ name, ip_address: ipAddress, bandwidth, cost }),
  });
export const listLinks = () => request<any[]>("/topology/links");
export const createLink = (
  ifaceA: string,
  ifaceB: string,
  bandwidth: number,
  delayMs: number,
) =>
  request<{ id: string }>("/topology/links", {
    method: "POST",
    body: JSON.stringify({
      interface_a_id: ifaceA,
      interface_b_id: ifaceB,
      bandwidth,
      delay_ms: delayMs,
    }),
  });
export const deleteLink = (id: string) =>
  request(`/topology/links/${id}`, { method: "DELETE" });
export const setLinkState = (id: string, isUp: boolean) =>
  request(`/topology/links/${id}/state`, {
    method: "PUT",
    body: JSON.stringify({ is_up: isUp }),
  });
export const getTopologySnapshot = () => request<any>("/topology/snapshot");

// OSPF
export const getOspfLsdb = (routerId: string) =>
  request<any>(`/ospf/${routerId}/lsdb`);
export const getOspfNeighbors = (routerId: string) =>
  request<any>(`/ospf/${routerId}/neighbors`);
export const getOspfRoutes = (routerId: string) =>
  request<any>(`/ospf/${routerId}/routes`);

// BGP
export const getBgpNeighbors = (routerId: string) =>
  request<any>(`/bgp/${routerId}/neighbors`);
export const getBgpRoutes = (routerId: string) =>
  request<any>(`/bgp/${routerId}/routes`);
export const enableBgp = (routerId: string, localAsn: number) =>
  request(`/bgp/${routerId}/enable`, {
    method: "POST",
    body: JSON.stringify({ local_asn: localAsn }),
  });
export const addBgpNeighbor = (
  routerId: string,
  neighborIp: string,
  remoteAsn: number,
) =>
  request(`/bgp/${routerId}/neighbors`, {
    method: "POST",
    body: JSON.stringify({ neighbor_ip: neighborIp, remote_asn: remoteAsn }),
  });

// Traffic
export const listGenerators = () => request<any[]>("/traffic/generators");
export const createGenerator = (
  sourceRouterId: string,
  destPrefix: string,
  rateBps: number,
  isActive: boolean,
) =>
  request<{ id: string }>("/traffic/generators", {
    method: "POST",
    body: JSON.stringify({
      source_router_id: sourceRouterId,
      dest_prefix: destPrefix,
      rate_bps: rateBps,
      is_active: isActive,
    }),
  });
export const updateGenerator = (
  id: string,
  updates: { rate_bps?: number; is_active?: boolean; dest_prefix?: string },
) =>
  request(`/traffic/generators/${id}`, {
    method: "PUT",
    body: JSON.stringify(updates),
  });
export const deleteGenerator = (id: string) =>
  request(`/traffic/generators/${id}`, { method: "DELETE" });
export const getFlows = () => request<any[]>("/traffic/flows");
export const getLinkUtilization = () =>
  request<Record<string, number>>("/traffic/link-utilization");

// Simulation
export const getSimState = () => request<any>("/simulation/state");
export const startSim = () =>
  request("/simulation/start", { method: "POST" });
export const stopSim = () =>
  request("/simulation/stop", { method: "POST" });
export const stepSim = () =>
  request<any>("/simulation/step", { method: "POST" });
export const setTickRate = (ms: number) =>
  request("/simulation/tick-rate", {
    method: "PUT",
    body: JSON.stringify({ tick_rate_ms: ms }),
  });
export const saveSim = () =>
  request<any>("/simulation/save", { method: "POST" });
export const loadSample = () =>
  request("/simulation/load-sample", { method: "POST" });

// Policies
export const listPolicies = () => request<any>("/policies");
export const createPolicy = (policyText: string) =>
  request<{ id: string }>("/policies", {
    method: "POST",
    body: JSON.stringify({ policy_text: policyText }),
  });
export const deletePolicy = (name: string) =>
  request(`/policies/${name}`, { method: "DELETE" });
