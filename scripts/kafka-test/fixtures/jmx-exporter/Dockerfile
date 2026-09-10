FROM eclipse-temurin:25-jre

ADD https://github.com/prometheus/jmx_exporter/releases/download/1.6.0/jmx_prometheus_standalone-1.6.0.jar /opt/jmx_prometheus_standalone-1.6.0.jar

RUN echo "6314f19186eb97023d424f5bdf108269fd86dbb0c70f65809b4d8751d68842b9  /opt/jmx_prometheus_standalone-1.6.0.jar" | sha256sum -c -

COPY exporter.yaml /opt/jmx-exporter.yaml

ENTRYPOINT ["java", "-jar", "/opt/jmx_prometheus_standalone-1.6.0.jar", "0.0.0.0:9404", "/opt/jmx-exporter.yaml"]
