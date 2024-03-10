FROM clux/muslrust
WORKDIR /app
# silly docker copy about directory
# sh -c 'cd .. && docker build --network host -t demo -f actixweb-sqlx-jwt/Dockerfile .'
COPY . . 
RUN ls -lah && RUSTFLAGS='-C target-feature=+crt-static' cargo build --release && ls -lah target/*

FROM alpine:latest  
RUN apk --no-cache add ca-certificates
WORKDIR /opt/
COPY --from=0 /app/target/x86_64-unknown-linux-musl/release/maggie /usr/bin/maggie
CMD ["maggie"]
