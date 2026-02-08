FROM node:24-alpine AS build

WORKDIR /app
COPY package.json package-lock.json ./
RUN npm install

COPY . .
RUN npm run build

FROM nginx:alpine

# Default port if none is provided
ENV PORT=80
EXPOSE ${PORT}

# Default BACKEND_URL if not provided (Note the trailing slash!)
ENV BACKEND_URL=http://sfs-backend:8000/

# The Nginx entrypoint will automatically process this template
COPY nginx.conf.template /etc/nginx/templates/default.conf.template
COPY --from=build /app/dist /usr/share/nginx/html
