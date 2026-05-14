#![allow(clippy::disallowed_methods)]

//! Phase 05 acceptance test driving the SFU directly (without the signaling
//! layer). Builds a webrtc-rs client peer, creates an offer with a recvonly
//! audio transceiver, hands it to `Sfu::on_offer`, sets the returned SDP as
//! the client's remote description. Proves the SFU completes a full SDP
//! round trip with a real webrtc-rs peer.

use meet_core::signaling::sfu_api::SfuPort;
use meet_sfu::api_engine;
use meet_sfu::Sfu;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;

const ROOM: &str = "room-1";

async fn fresh_client_pc() -> webrtc::peer_connection::RTCPeerConnection {
    let api = api_engine::build(None).expect("api");
    api.new_peer_connection(RTCConfiguration::default())
        .await
        .expect("new_peer_connection")
}

#[tokio::test(flavor = "multi_thread")]
async fn full_offer_answer_round_trip() {
    let sfu = Sfu::new_default().expect("sfu");

    sfu.on_join(ROOM, "alice").await.expect("join");

    let client = fresh_client_pc().await;
    client
        .add_transceiver_from_kind(
            webrtc::rtp_transceiver::rtp_codec::RTPCodecType::Audio,
            Some(RTCRtpTransceiverInit {
                direction: RTCRtpTransceiverDirection::Recvonly,
                send_encodings: Vec::new(),
            }),
        )
        .await
        .expect("add transceiver");

    let offer = client.create_offer(None).await.expect("create_offer");
    client
        .set_local_description(offer.clone())
        .await
        .expect("set_local_description");
    // Wait for ICE gathering so the offer has candidates.
    let mut gather = client.gathering_complete_promise().await;
    let _ = gather.recv().await;
    let final_offer = client.local_description().await.expect("local desc");

    // Hand the offer to the SFU; expect a valid answer back.
    let answer_sdp = sfu
        .on_offer(ROOM, "alice", &final_offer.sdp)
        .await
        .expect("on_offer");

    assert!(!answer_sdp.is_empty(), "answer SDP must not be empty");
    assert!(
        answer_sdp.contains("v=0"),
        "answer must start with SDP version line"
    );
    assert!(
        answer_sdp.contains("a=ice-pwd:") || answer_sdp.contains("a=ice-ufrag:"),
        "answer must carry ICE credentials"
    );

    // Apply it on the client side — proves the SDP is wire-valid.
    let answer = RTCSessionDescription::answer(answer_sdp).expect("answer parse");
    client
        .set_remote_description(answer)
        .await
        .expect("set_remote_description");

    // Cleanup.
    let _ = client.close().await;
    sfu.on_leave(ROOM, "alice").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn offer_for_unknown_participant_fails() {
    let sfu = Sfu::new_default().expect("sfu");
    let err = sfu.on_offer("nosuchroom", "ghost", "v=0\r\n").await;
    assert!(err.is_err(), "must reject unknown participants");
}

#[tokio::test(flavor = "multi_thread")]
async fn fifteen_participants_can_join() {
    let sfu = Sfu::new_default().expect("sfu");
    let pids: Vec<String> = (0..15).map(|i| format!("p{i}")).collect();
    for p in &pids {
        sfu.on_join("loadtest", p).await.expect("join");
    }
    for p in &pids {
        sfu.on_leave("loadtest", p).await;
    }
}
