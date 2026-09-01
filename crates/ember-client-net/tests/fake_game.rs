use std::time::Duration;

use ember_client_net::{
    AcknowledgementMode, CorrectionMode, HookError, InnerFrameCodec, PredictionHooks,
    Reconciler, RemoteEntityHooks, RemoteSnapshotBuffer, ReplayContext, WireFrame,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct FakeInput(i32);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FakeState {
    position: i32,
    acknowledgement: u32,
    server_timestamp: u64,
}

struct FakeGame;

impl InnerFrameCodec for FakeGame {
    type Outbound = FakeInput;
    type Inbound = FakeState;

    fn encode_inner(&self, message: &Self::Outbound) -> Result<WireFrame, HookError> {
        Ok(WireFrame::Text(message.0.to_string()))
    }

    fn decode_inner(&self, frame: &WireFrame) -> Result<Self::Inbound, HookError> {
        let WireFrame::Text(text) = frame else {
            return Err(HookError::WrongFrameKind);
        };
        text.parse::<i32>()
            .map(|position| FakeState {
                position,
                acknowledgement: 0,
                server_timestamp: 0,
            })
            .map_err(|error| HookError::Decode(error.to_string()))
    }
}

impl PredictionHooks for FakeGame {
    type Input = FakeInput;
    type AuthoritativeState = FakeState;
    type PredictedState = i32;

    fn acknowledgement(&self, authoritative: &Self::AuthoritativeState) -> u32 {
        authoritative.acknowledgement
    }

    fn server_timestamp(&self, authoritative: &Self::AuthoritativeState) -> u64 {
        authoritative.server_timestamp
    }

    fn acknowledgement_mode(&self) -> AcknowledgementMode {
        AcknowledgementMode::Through
    }

    fn apply_authoritative(
        &self,
        predicted: &mut Self::PredictedState,
        authoritative: &Self::AuthoritativeState,
    ) {
        *predicted = authoritative.position;
    }

    fn replay_one_slice(
        &self,
        predicted: &mut Self::PredictedState,
        input: &Self::Input,
        _context: ReplayContext,
        _authoritative: &Self::AuthoritativeState,
    ) {
        *predicted += input.0;
    }

    fn snap_or_smooth(
        &self,
        _before: &Self::PredictedState,
        _after: &Self::PredictedState,
        _authoritative: &Self::AuthoritativeState,
    ) -> CorrectionMode {
        CorrectionMode::Smooth
    }
}

impl RemoteEntityHooks for FakeGame {
    type Snapshot = i32;
    type RenderState = i32;

    fn interpolate_remote(
        &self,
        from: &Self::Snapshot,
        to: &Self::Snapshot,
        numerator: u64,
        denominator: u64,
    ) -> Self::RenderState {
        let numerator = i32::try_from(numerator).unwrap_or(i32::MAX);
        let denominator = i32::try_from(denominator).unwrap_or(i32::MAX).max(1);
        *from + (*to - *from) * numerator / denominator
    }

    fn dead_reckon_remote(
        &self,
        latest: &Self::Snapshot,
        elapsed: u64,
    ) -> Self::RenderState {
        *latest + i32::try_from(elapsed).unwrap_or(i32::MAX)
    }

    fn snap_or_smooth_remote(
        &self,
        _from: &Self::Snapshot,
        _to: &Self::Snapshot,
    ) -> CorrectionMode {
        CorrectionMode::Smooth
    }
}

#[test]
fn one_fake_game_satisfies_codec_prediction_and_remote_contracts() {
    let game = FakeGame;
    let frame = game.encode_inner(&FakeInput(7)).unwrap();
    assert_eq!(game.decode_inner(&frame).unwrap().position, 7);

    let mut prediction = Reconciler::new(4);
    prediction.record(FakeInput(3), Duration::from_millis(10));
    prediction.record(FakeInput(4), Duration::from_millis(20));
    let authoritative = FakeState {
        position: 100,
        acknowledgement: 1,
        server_timestamp: 55,
    };
    let mut predicted = 7;
    let reconciliation = prediction.reconcile(
        &game,
        &mut predicted,
        &authoritative,
        Duration::from_millis(30),
    );
    assert_eq!(predicted, 104);
    assert_eq!(reconciliation.replayed_inputs, 1);
    assert_eq!(reconciliation.correction, CorrectionMode::Smooth);

    let mut remote = RemoteSnapshotBuffer::new(2);
    remote.push(10, 0);
    remote.push(20, 20);
    assert_eq!(remote.sample_at(&game, 15), Some(10));
    assert_eq!(remote.sample_at(&game, 22), Some(22));
}
