# Stage 1: Build the application
FROM rust:1.90 as builder

# Create a new empty shell project
WORKDIR /usr/src/nicktd
COPY . .

# Change to the server directory to build the server crate
WORKDIR /usr/src/nicktd/server
RUN cargo build --release

# Stage 2: Create the final, smaller image
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl-dev && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /usr/src/app

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/nicktd/server/target/release/server .

# Expose the port the server runs on
EXPOSE 9001

# Set the startup command to run the binary
CMD ["./server"]
