FROM nginx

RUN mkdir -p /usr/share/nginx/html/client/dist
COPY js/client/index.html /usr/share/nginx/html/client/
COPY js/client/dist/* /usr/share/nginx/html/client/dist/

RUN mkdir -p /usr/share/nginx/html/explorer/dist
COPY js/explorer/index.html /usr/share/nginx/html/explorer/
COPY js/explorer/dist/* /usr/share/nginx/html/explorer/dist/

COPY docker/nginx/nginx.conf /etc/nginx/
