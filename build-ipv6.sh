#!/bin/bash
# MooseFS IPv6 Build Script
# Author: Benjamin Arntzen <zorlin@gmail.com>
# This script builds MooseFS with full IPv6 support

set -e

echo "==================================="
echo "MooseFS IPv6 Support Build Script"
echo "Author: Benjamin Arntzen <zorlin@gmail.com>"
echo "==================================="
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}[+]${NC} $1"
}

print_error() {
    echo -e "${RED}[!]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[*]${NC} $1"
}

# Check if we're in the MooseFS directory
if [ ! -f "configure.ac" ] || [ ! -d "mfscommon" ]; then
    print_error "This script must be run from the MooseFS source directory"
    exit 1
fi

print_status "Starting MooseFS IPv6 build process..."

# Step 1: Apply patches to build system
print_status "Step 1: Patching build system for IPv6 support..."

# Patch configure.ac directly with sed
if ! grep -q "ENABLE_IPV6" configure.ac; then
    print_status "Patching configure.ac for IPv6 support..."
    
    # Find the line with WORDS_BIGENDIAN and add IPv6 configuration after it
    sed -i '/^AC_C_BIGENDIAN/a\
\
dnl IPv6 Support - Benjamin Arntzen <zorlin@gmail.com>\
AC_ARG_ENABLE([ipv6],\
    AS_HELP_STRING([--enable-ipv6], [Enable IPv6 support (experimental)]),\
    [enable_ipv6=$enableval],\
    [enable_ipv6=no])\
\
if test "x$enable_ipv6" = "xyes"; then\
    AC_DEFINE([ENABLE_IPV6], [1], [Define to 1 to enable IPv6 support])\
fi\
AM_CONDITIONAL([ENABLE_IPV6], [test "x$enable_ipv6" = "xyes"])' configure.ac
else
    print_warning "configure.ac already patched for IPv6"
fi

# Step 2: Update Makefile.am files to include IPv6 sources
print_status "Step 2: Updating Makefile.am files..."

# Update mfschunkserver/Makefile.am
if ! grep -q "sockets_ipv6" mfschunkserver/Makefile.am; then
    print_status "Patching mfschunkserver/Makefile.am..."
    sed -i '/\.\.\/mfscommon\/sockets\.c \.\.\/mfscommon\/sockets\.h/a\
	../mfscommon/sockets_ipv6.c ../mfscommon/sockets_ipv6.h \\' mfschunkserver/Makefile.am
fi

# Update mfsmaster/Makefile.am
if ! grep -q "sockets_ipv6" mfsmaster/Makefile.am; then
    print_status "Patching mfsmaster/Makefile.am..."
    sed -i '/\.\.\/mfscommon\/sockets\.c \.\.\/mfscommon\/sockets\.h/a\
	../mfscommon/sockets_ipv6.c ../mfscommon/sockets_ipv6.h \\' mfsmaster/Makefile.am
fi

# Update mfsmetalogger/Makefile.am
if ! grep -q "sockets_ipv6" mfsmetalogger/Makefile.am; then
    print_status "Patching mfsmetalogger/Makefile.am..."
    sed -i '/\.\.\/mfscommon\/sockets\.c \.\.\/mfscommon\/sockets\.h/a\
	../mfscommon/sockets_ipv6.c ../mfscommon/sockets_ipv6.h \\' mfsmetalogger/Makefile.am
fi

# Update mfsclient/Makefile.am if it exists
if [ -f "mfsclient/Makefile.am" ] && ! grep -q "sockets_ipv6" mfsclient/Makefile.am; then
    print_status "Patching mfsclient/Makefile.am..."
    sed -i '/\.\.\/mfscommon\/sockets\.c \.\.\/mfscommon\/sockets\.h/a\
	../mfscommon/sockets_ipv6.c ../mfscommon/sockets_ipv6.h \\' mfsclient/Makefile.am
fi

# Step 3: Add IPv6 compile flags
print_status "Step 3: Adding IPv6 compile flags..."

for makefile in mfschunkserver/Makefile.am mfsmaster/Makefile.am mfsmetalogger/Makefile.am; do
    if [ -f "$makefile" ]; then
        if ! grep -q "ENABLE_IPV6" "$makefile"; then
            print_status "Adding IPv6 flags to $makefile..."
            cat >> "$makefile" << 'EOF'

# IPv6 Support
if ENABLE_IPV6
AM_CPPFLAGS += -DENABLE_IPV6
endif
EOF
        fi
    fi
done

# Step 4: Regenerate build files
print_status "Step 4: Regenerating build files..."

if [ -f "autogen.sh" ]; then
    print_status "Running autogen.sh..."
    ./autogen.sh
else
    print_status "Running autoreconf..."
    autoreconf -fi
fi

# Step 5: Configure with IPv6 support
print_status "Step 5: Configuring MooseFS with IPv6 support..."

./configure \
    --prefix=/usr \
    --sysconfdir=/etc \
    --localstatedir=/var/lib \
    --with-default-user=mfs \
    --with-default-group=mfs \
    --enable-ipv6 \
    CFLAGS="-O2 -g -DENABLE_IPV6" \
    CXXFLAGS="-O2 -g -DENABLE_IPV6"

# Step 6: Build
print_status "Step 6: Building MooseFS..."
make -j$(nproc)

print_status "Build complete!"
print_status ""
print_status "To install MooseFS with IPv6 support, run:"
echo "    sudo make install"
print_status ""
print_status "To test in Docker, run:"
echo "    docker-compose -f docker-compose.ipv6.yml up --build"
print_status ""
print_status "IPv6 configuration example:"
echo "    # In mfsmaster.cfg:"
echo "    MATOCS_LISTEN_HOST = ::"
echo "    MATOCL_LISTEN_HOST = ::"
echo ""
echo "    # In mfschunkserver.cfg:"
echo "    CSSERV_LISTEN_HOST = ::"
echo "    MASTER_HOST = 2001:db8::1"