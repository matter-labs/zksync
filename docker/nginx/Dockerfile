FROM nginx

RUN mkdir -p /usr/share/nginx/html/explorer/dist
COPY infrastructure/explorer/dist/ /usr/share/nginx/html/explorer/

COPY docker/nginx/nginx.conf /etc/nginx/nginx.conf
