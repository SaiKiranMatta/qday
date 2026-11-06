# The QDay web wallet, built for the hosted devnet (deploy/coolify) and served as static files by nginx.
# Build from the repository root: docker build -f deploy/web/Dockerfile .
FROM node:24-alpine AS build
RUN npm install -g pnpm@12.11.2
WORKDIR /app
# Dependencies first, so source changes don't reinstall them.
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY web/package.json web/
COPY packages/device/package.json packages/device/
COPY packages/frame/package.json packages/frame/
RUN pnpm install --frozen-lockfile
COPY packages packages
COPY web web
# local, hosted or public: see web/src/lib/chain.ts.
ARG VITE_QDAY_NETWORK=hosted
ENV VITE_QDAY_NETWORK=$VITE_QDAY_NETWORK
RUN pnpm --filter @qday/web build

FROM nginx:1.27-alpine
COPY deploy/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/web/dist /usr/share/nginx/html
EXPOSE 80
