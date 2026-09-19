FROM python:3.12.11-alpine3.22@sha256:efcdfa6a6b2fd2afb9c7dfa9a5b288a6f68338b5cfdebe6b637d986067d85757

WORKDIR /app

RUN adduser -D -u 10001 api-test
COPY server.py /app/server.py
RUN chown -R api-test:api-test /app

USER api-test
EXPOSE 8080

CMD ["python", "/app/server.py"]
