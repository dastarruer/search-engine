# Use rust base image
FROM rust:latest

# Cd into src
WORKDIR /usr/src/tf-idf-indexer

# Copy the src code into the container
COPY ./services .

COPY ./migrations /usr/src/migrations

# Install the application
RUN cargo install --path ./tf-idf-indexer

# Run the application
# CMD is the command used to start the container, while RUN is used to set up the container
CMD [ "tf-idf-indexer" ]
