/// Generated gRPC/protobuf code for the location service.
/// Handles City Simulator <-> Assignment Optimizer communication.
pub mod location {
    tonic::include_proto!("location");
}

/// Generated gRPC/protobuf code for the order service.
/// Handles Order Generator -> Assignment Optimizer communication.
pub mod order {
    tonic::include_proto!("order");
}

/// Generated gRPC/protobuf code for the assignment service.
/// Handles Assignment Optimizer -> Event Collector communication.
pub mod assignment {
    tonic::include_proto!("assignment");
}
