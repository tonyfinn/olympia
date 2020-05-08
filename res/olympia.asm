SECTION "Main", ROM0

Fizz EQU $F1
Buzz EQU $B0
FizzBuzz EQU $FB

; $0000 - $003F: RST handlers.
ret
REPT 7    
        nop
ENDR

; $0008
ret
REPT 7
    nop
ENDR

; $0010
ret
REPT 7
    nop
ENDR

; $0018
ret
REPT 7
    nop
ENDR

; $0020
ret
REPT 7
    nop
ENDR

; $0028
ret
REPT 7
    nop
ENDR

; $0030
ret
REPT 7
    nop
ENDR

; $0038
ret
REPT 7
    nop
ENDR

; $0040 - $0067: Interrupt handlers.
RETI
REPT 7
    nop
ENDR

; $0048
RETI
REPT 7
    nop
ENDR

; $0050
RETI
REPT 7
    nop
ENDR

; $0058
RETI
REPT 7
    nop
ENDR

; $0060
RETI
REPT 7
    nop
ENDR

; $0068 - $00FF: Free space.
DS $98

; $0100 - $0103: Startup handler.
nop
jp init

; $0104 - $0133: The Nintendo Logo.
DB $CE, $ED, $66, $66, $CC, $0D, $00, $0B
DB $03, $73, $00, $83, $00, $0C, $00, $0D
DB $00, $08, $11, $1F, $88, $89, $00, $0E
DB $DC, $CC, $6E, $E6, $DD, $DD, $D9, $99
DB $BB, $BB, $67, $63, $6E, $0E, $EC, $CC
DB $DD, $DC, $99, $9F, $BB, $B9, $33, $3E

; $0134 - $013E: The title, in upper-case letters, followed by zeroes.
DB "TONY"
DS 7 ; padding

; $013F - $0142: The manufacturer code.
DS 4


; $0143: Gameboy Color compatibility flag.
GBC_UNSUPPORTED EQU $00
GBC_COMPATIBLE EQU $80
GBC_EXCLUSIVE EQU $C0
DB GBC_UNSUPPORTED

; $0144 - $0145: "New" Licensee Code, a two character name.
DB "OK"

; $0146: Super Gameboy compatibility flag.
SGB_UNSUPPORTED EQU $00
SGB_SUPPORTED EQU $03
DB SGB_UNSUPPORTED

; $0147: Cartridge type. Either no ROM or MBC5 is recommended.
CART_ROM_ONLY EQU $00
CART_MBC1 EQU $01
CART_MBC1_RAM EQU $02
CART_MBC1_RAM_BATTERY EQU $03
CART_MBC2 EQU $05
CART_MBC2_BATTERY EQU $06
CART_ROM_RAM EQU $08
CART_ROM_RAM_BATTERY EQU $09
CART_MMM01 EQU $0B
CART_MMM01_RAM EQU $0C
CART_MMM01_RAM_BATTERY EQU $0D
CART_MBC3_TIMER_BATTERY EQU $0F
CART_MBC3_TIMER_RAM_BATTERY EQU $10
CART_MBC3 EQU $11
CART_MBC3_RAM EQU $12
CART_MBC3_RAM_BATTERY EQU $13
CART_MBC4 EQU $15
CART_MBC4_RAM EQU $16
CART_MBC4_RAM_BATTERY EQU $17
CART_MBC5 EQU $19
CART_MBC5_RAM EQU $1A
CART_MBC5_RAM_BATTERY EQU $1B
CART_MBC5_RUMBLE EQU $1C
CART_MBC5_RUMBLE_RAM EQU $1D
CART_MBC5_RUMBLE_RAM_BATTERY EQU $1E
CART_POCKET_CAMERA EQU $FC
CART_BANDAI_TAMA5 EQU $FD
CART_HUC3 EQU $FE
CART_HUC1_RAM_BATTERY EQU $FF
DB CART_ROM_ONLY

; $0148: Rom size.
ROM_32K EQU $00
ROM_64K EQU $01
ROM_128K EQU $02
ROM_256K EQU $03
ROM_512K EQU $04
ROM_1024K EQU $05
ROM_2048K EQU $06
ROM_4096K EQU $07
ROM_1152K EQU $52
ROM_1280K EQU $53
ROM_1536K EQU $54
DB ROM_32K

; $0149: Ram size.
RAM_NONE EQU $00
RAM_2K EQU $01
RAM_8K EQU $02
RAM_32K EQU $03
DB RAM_NONE

; $014A: Destination code.
DEST_JAPAN EQU $00
DEST_INTERNATIONAL EQU $01
DB DEST_INTERNATIONAL

; $014B: Old licensee code.
; $33 indicates new license code will be used.
; $33 must be used for SGB games.
DB $33
; $014C: ROM version number
DB $00
; $014D: Header checksum.
; Assembler needs to patch this.
DB $FF
; $014E- $014F: Global checksum.
; Assembler needs to patch this.
DW $FACE

init:
    LD A, 0
    LD B, 0
    LD C, 0
    LD D, 0
    LD E, 0
    LD H, 0
    LD L, 0
    CCF

    LD A, $11
    LD [$FF40], A

    LD HL, $8010
    LD BC, font
    LD D, 113
    CALL memcpy
    LD HL, $9824
    LD A, 1
    LD [HL+], A
    INC A
    LD [HL+], A
    INC A
    LD [HL+], A
    INC A
    LD [HL+], A
    INC A
    LD [HL+], A
    INC A
    LD [HL+], A
    INC A
    LD [HL+], A
    INC A
    LD A, $91
    LD [$FF40], A
    LD A, $E4
    LD [$FF47], A
    JP end

; Args
; HL Address to copy to
; BC Address to copy from
; D Copy size
memcpy:
    dec D

memcpy.loop:
    LD A, [BC]
    LD [HL+], A
    INC BC
    DEC D

memcpy.complete:
    LD A, D
    CP $00
    JR NZ, memcpy.loop
    RET


font:
font.o:
    DW `03333300
    DW `33333330
    DW `33000030
    DW `33000030
    DW `33000030
    DW `33000030
    DW `33333330
    DW `03333300
font.l:
    DW `33000000
    DW `33000000
    DW `33000000
    DW `33000000
    DW `33000000
    DW `33000000
    DW `33333330
    DW `33333330
font.y:
    DW `03300330
    DW `03300330
    DW `03300330
    DW `00333300
    DW `00033000
    DW `00033000
    DW `00033000
    DW `00033000
font.m:
    DW `33000330
    DW `33000330
    DW `33303330
    DW `30333030
    DW `30030030
    DW `30000030
    DW `30000030
    DW `30000030
font.p:
    DW `33333300
    DW `33000030
    DW `33000030
    DW `33333300
    DW `33000000
    DW `33000000
    DW `33000000
    DW `33000000
font.i:
    DW `00033000
    DW `00033000
    DW `00033000
    DW `00033000
    DW `00033000
    DW `00033000
    DW `00033000
    DW `00033000
font.a:
    DW `00333300
    DW `03300330
    DW `03300330
    DW `03333330
    DW `03300330
    DW `03300330
    DW `03300330
    DW `03300330


end:
    HALT
    JR end