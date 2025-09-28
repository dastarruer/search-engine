# Use rust base image
FROM rust:latest

# Cd into src
WORKDIR /usr/src/crawler

# Copy the src code into the container
COPY . .

# Install the application
RUN cargo install --path .

# Run the application
# CMD is the command used to start the container, while RUN is used to set up the container
CMD [ "crawler" ]
