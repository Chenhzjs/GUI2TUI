FROM debian:bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates python3 python3-pyatspi python3-pyte \
    dbus-x11 at-spi2-core xvfb openbox firefox-esr \
    ffmpeg xterm wmctrl fonts-dejavu-core \
    && rm -rf /var/lib/apt/lists/*
ENV LANG=C.UTF-8 LC_ALL=C.UTF-8
WORKDIR /tmp
