pub mod blockstore_adapter;
pub mod car;
pub mod operations;

// Re-export key types for external consumers
pub use blockstore_adapter::{RepoStoreAdapter, cid_from_bytes, cid_to_bytes};
pub use car::{export_full_car, generate_diff_car};
pub use operations::{
    RecordOutput, RecordWriteOutput, create_record, create_repo, delete_record, get_record,
    list_records, put_record,
};
