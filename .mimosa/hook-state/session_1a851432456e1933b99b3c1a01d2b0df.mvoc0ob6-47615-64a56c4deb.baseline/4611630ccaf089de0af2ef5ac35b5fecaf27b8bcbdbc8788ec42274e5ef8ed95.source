//! Dump dos ops do descritor do Task gerado (debug do crash de key serialization).
//! Compara com o ground truth do idlc C (OrchestratorV4.c):
//!   18 ADR x 2 words = 36 words, RTS no word 36, KOF|1,0 no word 37.

use cyclonedds::DdsType;
use dds_contract::generated::dds_llm_orchestrator::Task;

fn main() {
    let ops = <Task as DdsType>::ops();
    println!("type_name = {}", <Task as DdsType>::type_name());
    println!("nops(words) = {}", ops.len());
    for (i, op) in ops.iter().enumerate() {
        println!("  word {i:2}: 0x{op:08x}");
    }
    println!("key_count = {}", <Task as DdsType>::key_count());
    for k in <Task as DdsType>::keys() {
        println!("key: name={:?} ops_path={:?}", k.name, k.ops_path);
    }
    let pko = <Task as DdsType>::post_key_ops();
    println!("post_key_ops ({} words):", pko.len());
    for (i, op) in pko.iter().enumerate() {
        println!("  pko {i:2}: 0x{op:08x}");
    }
    println!("descriptor_size = {}", <Task as DdsType>::descriptor_size());
    println!(
        "descriptor_align = {}",
        <Task as DdsType>::descriptor_align()
    );
}
