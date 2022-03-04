FROM ubuntu:18.04
RUN apt-get update && apt-get -yy install curl openssl
ENV mirversion="1.3.0"
RUN curl -L -o full-service.tar.gz https://github.com/mobilecoinofficial/full-service-mirror/releases/download/v$mirversion/v$mirversion-testnet.tar.gz
RUN tar xf full-service.tar.gz && rm full-service.tar.gz
WORKDIR /v$mirversion-testnet
COPY ./run.sh .
ENTRYPOINT ["/bin/bash", "./run.sh"]
