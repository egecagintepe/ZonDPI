ifndef MSYSTEM
	CPREFIX = x86_64-w64-mingw32-
endif

WINDIVERTHEADERS ?= ../../../../third_party/windivert/include
WINDIVERTLIBS ?= ../../../../third_party/windivert/x64

TARGET = goodbyedpi.exe
LIBS = -L$(WINDIVERTLIBS) -lWinDivert -lws2_32 -l:libssp.a
CC = $(CPREFIX)gcc

CCWINDRES = $(CPREFIX)windres
ifeq (, $(shell which $(CPREFIX)windres 2>nul))
	CCWINDRES = windres
endif

CFLAGS = -std=c99 -pipe -I$(WINDIVERTHEADERS) -L$(WINDIVERTLIBS) \
         -O2 -D_FORTIFY_SOURCE=2 -fstack-protector \
         -Wall -Wextra -Wpedantic -Wformat=2 \
         -Wformat-security -Wno-format-nonliteral -Wshadow -Wstrict-aliasing=1 \
         -Wnull-dereference -Warray-bounds=2 -Wimplicit-fallthrough=3 \
         -Wformat-signedness -Wstrict-overflow=2 -Wcast-align=strict \
         -Wfloat-equal -Wcast-align -Wsign-conversion

LDFLAGS = -fstack-protector -Wl,-O1,--dynamicbase,--nxcompat,--sort-common,--as-needed -Wl,--high-entropy-va

ifdef BIT64
	LDFLAGS += -Wl,--high-entropy-va
else
	CFLAGS += -m32
	LDFLAGS += -m32
endif

.PHONY: default all clean

default: $(TARGET)
all: default

OBJECTS = blackwhitelist.o dnsredir.o fakepackets.o goodbyedpi.o service.o ttltrack.o utils/getline.o utils/repl_str.o goodbyedpi-rc.o
HEADERS = $(wildcard *.h utils/*.h)

%.o: %.c $(HEADERS)
	$(CC) $(CFLAGS) -c $< -o $@

goodbyedpi-rc.o: goodbyedpi-rc.rc
	$(CCWINDRES) goodbyedpi-rc.rc -o goodbyedpi-rc.o

.PRECIOUS: $(TARGET) $(OBJECTS)

$(TARGET): $(OBJECTS)
	$(CC) $(OBJECTS) $(LDFLAGS) $(LIBS) -s -o $@

clean:
	-cmd /c del /f /q *.o utils\*.o $(TARGET) 2>nul || rm -f *.o utils/*.o $(TARGET)
