use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    LinkStateChange {
        link_id: String,
        is_up: bool,
    },
    LsaFlood {
        lsa_key: String,
        source_router_id: String,
        area_id: u32,
    },
    BgpSessionEvent {
        router_id: String,
        neighbor_ip: String,
        event: BgpFsmEvent,
    },
    BgpUpdate {
        router_id: String,
        neighbor_ip: String,
    },
    TrafficStart {
        generator_id: String,
    },
    TrafficStop {
        generator_id: String,
    },
    PolicyChange {
        policy_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BgpFsmEvent {
    Start,
    TcpEstablished,
    OpenReceived,
    KeepaliveReceived,
    HoldTimerExpired,
    Stop,
}

#[derive(Debug, Clone)]
pub struct SimEvent {
    pub scheduled_tick: u64,
    pub priority: u32,
    pub event_type: EventType,
}

impl PartialEq for SimEvent {
    fn eq(&self, other: &Self) -> bool {
        self.scheduled_tick == other.scheduled_tick && self.priority == other.priority
    }
}

impl Eq for SimEvent {}

// BinaryHeap is a max-heap, so we reverse ordering for min-heap behavior
impl PartialOrd for SimEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SimEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .scheduled_tick
            .cmp(&self.scheduled_tick)
            .then_with(|| other.priority.cmp(&self.priority))
    }
}

#[derive(Debug, Default)]
pub struct EventQueue {
    heap: BinaryHeap<SimEvent>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            heap: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, event: SimEvent) {
        self.heap.push(event);
    }

    pub fn schedule(&mut self, tick: u64, priority: u32, event_type: EventType) {
        self.heap.push(SimEvent {
            scheduled_tick: tick,
            priority,
            event_type,
        });
    }

    pub fn drain_for_tick(&mut self, tick: u64) -> Vec<SimEvent> {
        let mut events = Vec::new();
        while let Some(ev) = self.heap.peek() {
            if ev.scheduled_tick <= tick {
                events.push(self.heap.pop().unwrap());
            } else {
                break;
            }
        }
        events
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}
