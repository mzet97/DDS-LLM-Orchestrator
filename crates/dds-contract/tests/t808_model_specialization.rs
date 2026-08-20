#![cfg(feature = "dds")]

use cyclonedds::{CdrDeserializer, CdrEncoding, CdrSerializer, DdsEnumType, DdsType};
use dds_contract::generated::dds_llm_orchestrator::{ModelSpecialization, Task};

#[test]
fn generated_model_specialization_matches_runtime_contract() {
    assert_eq!(ModelSpecialization::MS_TEXT as i32, 0);
    assert_eq!(ModelSpecialization::MS_VISION as i32, 1);
    assert_eq!(ModelSpecialization::MS_EMBEDDING as i32, 2);
    assert_eq!(ModelSpecialization::MS_TRANSCRIPTION as i32, 3);
    assert_eq!(ModelSpecialization::max_discriminant(), 3);
}

#[test]
fn transcription_discriminant_roundtrips_over_xcdr1_and_xcdr2() {
    let task = Task {
        task_id: "t808-xcdr".into(),
        model_required: ModelSpecialization::MS_TRANSCRIPTION as i32,
        ..Task::default()
    };
    for encoding in [CdrEncoding::Xcdr1, CdrEncoding::Xcdr2] {
        let bytes = CdrSerializer::serialize(&task, encoding).unwrap();
        let observed: Task = CdrDeserializer::deserialize(&bytes, encoding).unwrap();
        assert_eq!(observed.model_required, 3);
    }
}

#[test]
fn task_type_id_matches_generated_c_descriptor() {
    // Generated C reports MINIMAL 579d2a90f85a139ff6852f00baaa.
    // The ID stays stable because `Task.model_required` is the canonical long
    // wire field; the standalone enum definition documents/parses that value.
    const C_MINIMAL_TYPE_ID: [u8; 14] = [
        0x57, 0x9d, 0x2a, 0x90, 0xf8, 0x5a, 0x13, 0x9f, 0xf6, 0x85, 0x2f, 0x00, 0xba, 0xaa,
    ];
    let (type_info, _) = Task::type_metadata_blobs().unwrap();
    assert!(type_info
        .windows(C_MINIMAL_TYPE_ID.len())
        .any(|window| window == C_MINIMAL_TYPE_ID));
}
