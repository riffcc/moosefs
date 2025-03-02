# Minimal HA module Makefile.am

AUTOMAKE_OPTIONS = foreign

noinst_LIBRARIES = libmfsmaster_ha.a

libmfsmaster_ha_a_SOURCES = ../matoclserv_ha.c

install-data-local:
	@echo "HA module installed as part of mfsmaster"

all:

clean:

distclean-local: distclean-am
	-rm -rf ./$(DEPDIR)
	-rm -f Makefile 