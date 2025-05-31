# MooseFS Client Dockerfile
FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libc6-dev \
    libfuse3-dev \
    fuse3 \
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

# Build MooseFS client
RUN ./configure --prefix=/usr/local --sysconfdir=/etc/mfs --localstatedir=/var/lib/mfs \
    --with-default-user=mfs --with-default-group=mfs
RUN make -j$(nproc)
RUN make install

# Create necessary directories
RUN mkdir -p /var/lib/mfs /var/log/mfs /etc/mfs /mnt/mfs
RUN chown -R mfs:mfs /var/lib/mfs /var/log/mfs /etc/mfs

# Copy configuration files
COPY mfsdata/mfsmount.cfg.in /etc/mfs/mfsmount.cfg

# Set ownership of config files
RUN chown -R mfs:mfs /etc/mfs

# Set working directory
WORKDIR /var/lib/mfs

# Start as root to handle FUSE mounting, then drop privileges
USER root

# Start client with mount
CMD ["sh", "-c", "mkdir -p /mnt/mfs && mfsmount /mnt/mfs -H ${MFSMOUNT_MASTER_HOST:-master} -P ${MFSMOUNT_MASTER_PORT:-9419} -o allow_other,default_permissions && tail -f /dev/null"]