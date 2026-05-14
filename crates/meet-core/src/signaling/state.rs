//! Per-connection state machine. Pure functions so the transitions can be
//! tested without spinning up tokio + sockets.

use crate::signaling::CloseCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    /// Waiting for the first `Join` message.
    Connecting,
    /// Token verified, in-room state has been broadcast.
    InRoom,
    /// Terminal — the writer should drain and the socket should close.
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    JoinReceived,
    AuthOk,
    AuthFail,
    Tick,
    Idle,
    PeerMessage,
    Replaced,
    ProtocolViolation,
    TooLarge,
    Backpressure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Continue normally with the new state.
    Continue(ConnState),
    /// Close the socket with the given code.
    Close(ConnState, CloseCode),
}

#[must_use]
pub fn transition(state: ConnState, event: Event) -> Action {
    match (state, event) {
        // Auth path.
        (ConnState::Connecting, Event::JoinReceived) => Action::Continue(ConnState::Connecting),
        (ConnState::Connecting, Event::AuthOk) => Action::Continue(ConnState::InRoom),
        (ConnState::Connecting, Event::AuthFail) => {
            Action::Close(ConnState::Closed, CloseCode::AuthFailure)
        },

        // Steady-state ticks.
        (ConnState::InRoom, Event::PeerMessage | Event::Tick) => {
            Action::Continue(ConnState::InRoom)
        },

        // Idle / replaced / backpressure terminate from any non-closed state.
        (s, Event::Idle) if s != ConnState::Closed => {
            Action::Close(ConnState::Closed, CloseCode::IdleTimeout)
        },
        (s, Event::Replaced) if s != ConnState::Closed => {
            Action::Close(ConnState::Closed, CloseCode::Replaced)
        },
        (s, Event::Backpressure) if s != ConnState::Closed => {
            Action::Close(ConnState::Closed, CloseCode::Backpressure)
        },
        (s, Event::TooLarge) if s != ConnState::Closed => {
            Action::Close(ConnState::Closed, CloseCode::MessageTooLarge)
        },
        (s, Event::ProtocolViolation) if s != ConnState::Closed => {
            Action::Close(ConnState::Closed, CloseCode::ProtocolViolation)
        },

        // Anything else from a closed state is a no-op.
        (ConnState::Closed, _) => Action::Continue(ConnState::Closed),

        // Unexpected combinations (e.g. PeerMessage before Join) are protocol
        // violations.
        _ => Action::Close(ConnState::Closed, CloseCode::ProtocolViolation),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    #[test]
    fn happy_path() {
        let s = ConnState::Connecting;
        let a = transition(s, Event::JoinReceived);
        assert!(matches!(a, Action::Continue(ConnState::Connecting)));
        let a = transition(ConnState::Connecting, Event::AuthOk);
        assert!(matches!(a, Action::Continue(ConnState::InRoom)));
    }

    #[test]
    fn idle_closes_from_any_state() {
        assert!(matches!(
            transition(ConnState::Connecting, Event::Idle),
            Action::Close(ConnState::Closed, CloseCode::IdleTimeout)
        ));
        assert!(matches!(
            transition(ConnState::InRoom, Event::Idle),
            Action::Close(ConnState::Closed, CloseCode::IdleTimeout)
        ));
    }

    #[test]
    fn auth_fail_closes_4401() {
        assert!(matches!(
            transition(ConnState::Connecting, Event::AuthFail),
            Action::Close(_, CloseCode::AuthFailure)
        ));
    }

    #[test]
    fn protocol_violation_for_unexpected_event() {
        // PeerMessage while still Connecting (no Join yet) is a violation.
        assert!(matches!(
            transition(ConnState::Connecting, Event::PeerMessage),
            Action::Close(_, CloseCode::ProtocolViolation)
        ));
    }

    #[test]
    fn replaced_closes_4409() {
        assert!(matches!(
            transition(ConnState::InRoom, Event::Replaced),
            Action::Close(_, CloseCode::Replaced)
        ));
    }

    #[test]
    fn closed_is_terminal() {
        assert!(matches!(
            transition(ConnState::Closed, Event::PeerMessage),
            Action::Continue(ConnState::Closed)
        ));
    }
}
