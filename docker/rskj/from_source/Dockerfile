# Build RSKj jar
FROM openjdk:11-jdk-slim-buster as build

RUN apt-get update -y && \
    apt-get install -y git curl gnupg && \
    rm -rf /var/lib/apt/lists/* && \
    apt-get autoremove -y && \
    apt-get clean

RUN gpg --keyserver https://secchannel.rsk.co/release.asc --recv-keys 1A92D8942171AFA951A857365DECF4415E3B8FA4
RUN gpg --finger 1A92D8942171AFA951A857365DECF4415E3B8FA4

ARG rskj_codename=IRIS
ARG rskj_version=3.1.0
ENV RSKJ_BRANCH=IRIS_3_1_0

# The code is a fork of 3.1.0 where I removed JCentral repo which has been sunsetted
RUN git clone --single-branch --depth 1 --branch "${RSKJ_BRANCH}" https://github.com/raullaprida/rskj.git /code/rskj


WORKDIR /code/rskj

RUN gpg --verify SHA256SUMS.asc
RUN sha256sum --check SHA256SUMS.asc
RUN ./configure.sh
RUN ./gradlew --no-daemon clean build -x test

# Run RSKj
FROM ubuntu:20.04
ARG rskj_codename=IRIS
ARG rskj_version=3.1.0

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update
RUN apt-get install -y --no-install-recommends openjdk-11-jre supervisor systemd software-properties-common

COPY --from=build /code/rskj/rskj-core/build/libs/rskj-core-${rskj_version}-${rskj_codename}-all.jar /usr/share/rsk/rsk.jar

WORKDIR /usr/share/rsk

RUN mkdir /var/lib/rsk

RUN groupadd --gid 888 rsk && useradd rsk -d /var/lib/rsk -s /sbin/nologin --uid=888 --gid=888

RUN rm -f /etc/rsk/node.conf
RUN mkdir /var/lib/rsk/database && \
  chown rsk:rsk /var/lib/rsk/database

# Supervisod CONF
COPY supervisord.conf /etc/supervisor/conf.d/supervisord.conf
COPY node.conf /etc/rsk/node.conf
COPY logback.xml /etc/rsk/logback.xml

EXPOSE 4444
EXPOSE 50505

# Define default command.
CMD ["/usr/bin/supervisord"]
