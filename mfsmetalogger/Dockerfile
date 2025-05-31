# MooseFS Metalogger Dockerfile
FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libc6-dev \
    libfuse3-dev \
    python3 \
    python3-pip \
    zlib1g-dev \
    && rm -rf /var/lib/apt/lists/*

# Set build argument for build number
ARG BUILDNO=unknown

# Create mfs user and group
RUN groupadd -r mfs && useradd -r -g mfs -d /var/lib/mfs -s /bin/bash mfs

# Copy MooseFS source code
COPY . /usr/src/moosefs
WORKDIR /usr/src/moosefs

# Build MooseFS metalogger
RUN ./configure --prefix=/usr/local --sysconfdir=/etc/mfs --localstatedir=/var/lib/mfs \
    --with-default-user=mfs --with-default-group=mfs --disable-mfsmount
RUN make -j$(nproc)
RUN make install

# Create necessary directories
RUN mkdir -p /var/lib/mfs /var/log/mfs /etc/mfs
RUN chown -R mfs:mfs /var/lib/mfs /var/log/mfs /etc/mfs

# Copy configuration files
COPY mfsdata/mfsmetalogger.cfg.in /etc/mfs/mfsmetalogger.cfg

# Set ownership of config files
RUN chown -R mfs:mfs /etc/mfs

# Set working directory
WORKDIR /var/lib/mfs

# Switch to mfs user
USER mfs

# Health check
HEALTHCHECK --interval=60s --timeout=10s --start-period=30s --retries=3 \
    CMD mfsmetalogger -t || exit 1

# Start metalogger
CMD ["mfsmetalogger", "-f"]