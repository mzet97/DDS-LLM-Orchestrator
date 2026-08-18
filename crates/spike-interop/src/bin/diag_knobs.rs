// Diagnóstico: qual chamada falha ao aplicar knobs
use cyclonedds::*;
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_contract::topics;

fn main() {
    let dp = DomainParticipant::new(110).unwrap();
    let publisher = Publisher::new(&dp).unwrap();
    let qos = QosBuilder::new()
        .reliability(Reliability::Reliable, 10_000_000_000)
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(50))
        .ownership(Ownership::Exclusive)
        .liveliness(Liveliness::Automatic, 10_000_000_000)
        .latency_budget(50_000_000)
        .transport_priority(8)
        .ownership_strength(200)
        .build()
        .unwrap();
    let topic = Topic::<Task>::with_qos(&dp, topics::TASKS, Some(&qos)).unwrap();
    let writer = DataWriter::<Task>::with_qos(&publisher, &topic, Some(&qos)).unwrap();

    // 1) set_qos com o MESMO QoS (delta zero)
    println!("1) mesmo QoS: {:?}", writer.set_qos(&qos));

    // 2) set_qos só mudando transport_priority
    let qos2 = QosBuilder::new()
        .reliability(Reliability::Reliable, 10_000_000_000)
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(50))
        .ownership(Ownership::Exclusive)
        .liveliness(Liveliness::Automatic, 10_000_000_000)
        .latency_budget(50_000_000)
        .transport_priority(1)
        .ownership_strength(200)
        .build()
        .unwrap();
    println!("2) transport_priority 8→1: {:?}", writer.set_qos(&qos2));

    // 3) set_qos mudando ownership_strength
    let qos3 = QosBuilder::new()
        .reliability(Reliability::Reliable, 10_000_000_000)
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(50))
        .ownership(Ownership::Exclusive)
        .liveliness(Liveliness::Automatic, 10_000_000_000)
        .latency_budget(50_000_000)
        .transport_priority(1)
        .ownership_strength(100)
        .build()
        .unwrap();
    println!("3) ownership_strength 200→100: {:?}", writer.set_qos(&qos3));

    // 4) set_qos mudando latency_budget
    let qos4 = QosBuilder::new()
        .reliability(Reliability::Reliable, 10_000_000_000)
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(50))
        .ownership(Ownership::Exclusive)
        .liveliness(Liveliness::Automatic, 10_000_000_000)
        .latency_budget(20_000_000)
        .transport_priority(1)
        .ownership_strength(100)
        .build()
        .unwrap();
    println!("4) latency_budget 50ms→20ms: {:?}", writer.set_qos(&qos4));
}
