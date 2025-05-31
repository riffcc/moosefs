# MooseFS Chunk Server Dockerfile
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

# Build MooseFS chunkserver
RUN ./configure --prefix=/usr/local --sysconfdir=/etc/mfs --localstatedir=/var/lib/mfs \
    --with-default-user=mfs --with-default-group=mfs --disable-mfsmount
RUN make -j$(nproc)
RUN make install

# Create necessary directories
RUN mkdir -p /var/lib/mfs /var/log/mfs /etc/mfs
RUN mkdir -p /mnt/mfschunks1 /mnt/mfschunks2 /mnt/mfschunks3 /mnt/mfschunks4
RUN chown -R mfs:mfs /var/lib/mfs /var/log/mfs /etc/mfs /mnt/mfschunks*

# Copy configuration files
COPY mfsdata/mfschunkserver.cfg.in /etc/mfs/mfschunkserver.cfg

# Create HDD configuration file for 4 storage directories
RUN echo "/mnt/mfschunks1" > /etc/mfs/mfshdd.cfg && \
    echo "/mnt/mfschunks2" >> /etc/mfs/mfshdd.cfg && \
    echo "/mnt/mfschunks3" >> /etc/mfs/mfshdd.cfg && \
    echo "/mnt/mfschunks4" >> /etc/mfs/mfshdd.cfg

# Set ownership of config files
RUN chown -R mfs:mfs /etc/mfs

# Expose port
EXPOSE 9422

# Set working directory
WORKDIR /var/lib/mfs

# Switch to mfs user
USER mfs

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=15s --retries=3 \
    CMD mfschunkserver -t || exit 1

# Start chunk server
CMD ["mfschunkserver", "-f"]