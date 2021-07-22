FROM tcardonne/github-runner
FROM docker:dind
RUN apk update
RUN apk add py-pip python3-dev libffi-dev openssl-dev gcc libc-dev make
RUN pip install docker-compose
