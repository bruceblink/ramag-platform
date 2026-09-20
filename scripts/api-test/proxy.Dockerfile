FROM python:3.12.11-alpine3.22

WORKDIR /app
RUN adduser -D -u 10001 api-test
COPY proxy.py /app/proxy.py
RUN chown -R api-test:api-test /app
USER api-test

CMD ["python", "/app/proxy.py"]
