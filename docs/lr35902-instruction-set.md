# Register identifiers
## Standard 8-bit registers
000 - B
001 - C - used for 8 bit relative addressing
010 - D
011 - E
100 - H
101 - L
110 - (HL) - M Used for memory references
111 - A - Accumalator, used for result of untargeted instructions


## 16 bit registers including SP
aa = 00 -> BC
aa = 01 -> DE
aa = 10 -> HL
aa = 11 -> SP


## 16 bit registers including accumlator

00 -> BC
01 -> DE
10 -> HL
11 -> AF


## Flags

ZNHC0000

Z = Zero bit - set if result is 0
N = Subtract - 1 = < 0, 0 = > 0
H = Half carry (carry from bit 3 to bit 4, e.g. 0000 1000 + 0000 1000 = 0001 0000)
C = Carry flag - set if carry


## Math Operations
000 = ADD
001 = ADC
010 = SUB
011 = SBC
100 = AND
101 = XOR
110 = OR
111 = CP

# 00xx xxxx - Misc Area 1

## 000x x000 - Assorted instructions
### 0000 0000 - NOP
Size 1, timing 1


#### 0000 1000 - LD (a16), SP
Size 3, timing 20


### 0001 0000 - STOP 0
Size 2, timing 4


### 0001 1000 - Unconditional relative jump
Size 2, timing 12


## 001x x000 - Conditional relative jump
Size 2, timing 12/8

xx = 00 -> JR NZ (jump if non-zero)
xx = 01 -> JR Z (jump if zero)
xx = 10 -> JR NC (jump if not carry)
xx = 11 -> JR C (jump if carry flag)


## 00xx 0001 - MOV rr, d16 (16-bit constant)
Size 3, timing 12

00 aa 0001

aa = 16 bit register including SP


## 00xx 1001 - ADD 16-bit register to HL
Size 1, timing 8

00 aa 1001

aa = 16 bit register inccd 


## 00xx x010 = MOV (address in 16 bit register)
Size 1, timing 8

00 aa b 010

aa = 16 bit register identifer
b = 0 -> LD (aa), A
b = 1 -> LD A, (aa)

aa = 00 -> (BC)
aa = 01 -> (DE)
aa = 10 -> (HL+)
aa = 11 -> (HL-)


## 00xx x011 = INC/DEC 16 bit
Size 1, timing 8

00 aa b 011

aa = 16 bit register including SP

b = 0 -> INC
b = 1 -> DEC


## 00xx x100 = INC 8 bit
Size 1, timing 4

00 aaa 100

aaa = register identifier


## 00xx x101 DEC 8 bit
Size 1, timing 4

00 aaa 101

aaa = register identifier


## 00xx x110 = MOV constant
Size 2, timing 8

xxx = register identifier

0011 0110


## 000x x111 - Rotate instructions
Size 1, timing, 4

RLCA, RRCA, RLA, RRA

000 ab 111

a = carry (0 = carry, 1 = no carry)
 - if this is set the carry bit is rotated into the value of A, otherwise just the bits of A are rotated
b = direction (0 = left, 1 = right)


## 0010 x111 - Accumalator
Size 1, timing 4

x = 0 -> convert A to binary-coded decimal
x = 1 -> invert A


## 0011 x111 - Carry Flag
Size 1, timing 4

x = 0 -> set carry flag to 1
x = 1 -> invert carry flag


# 01xx xxxx = Register MOV
Size 1, timing 4 (unless sourcer or dest is (HL), then 8)

01 AAA BBB = MOV dest, srouce

MOV (HL), (HL) = HLT


# 10xx xxxx (Maths)
Size 1, timing 4 (unless source is (HL), then 8)

10 OOO SSS = Math operations (000 = Opeartion, SSS = Source, A = dest always)

Operations:

# 11xx xxxx (Misc area 2)

## 110x x000 - Conditional ret
Size 1, Timing 20/8

110 n f oo 0

n = 0 -> not eq
n = 1 -> equal

f = 0 -> zero flag
f = 1 -> carry flag

## 110x x010 - Conditional jump
Size 3, Timing 16/12

110 n f oo 0

n = 0 -> not eq
n = 1 -> equal

f = 0 -> zero flag
f = 1 -> carry flag

## 110x x100  - Conditional call
Size 3, Timing 24/12

110 n f oo 0

n = 0 -> not eq
n = 1 -> equal

f = 0 -> zero flag
f = 1 -> carry flag

## 1100 0011 - Unconditional jump to instruction param
Size 3, timing 16

## 11xx x110 - Math operations on constants
Size 2, timing 8

11 ooo 110

ooo = math operation

## 1111 0011 - Disable interrupts
Size 1, timing 4

## 1111 1011 - Enable interrupts
Size 1, timing 4

## 1100 1011 - Execute extended instruction
Size 1, timing 4

## 1100 1011 - Unconditional Ret
Size 1, timing 16

## 1100 1101 - Unconditional Call 
Size 3, timing 24

## 11xx x111 - Call system routines
Size 1, timing 16

11 ooo 111

(ooo << 3 = address)
000 = 0h
001 = 8h
010 = 10h
011 = 18h
100 = 20h
101 = 28h
110 = 30h
111 = 38h

## 1110 x010 - 8 bit offset accumaltor load
Size 2, timing 8

x = 0 -> LD (C), A
x = 1 -> LD A, (C)

## 1101 1011 - Uncondtional return from interrupt
Size 1, timing 16

## 1110 1011 - Unconditional jump to register address
Size 1, timing 4

## 1111 1011 - Load memory address to stack pointer
Size 1, timing 8

## 111x 0000 - Load High
Size 2, Timing 12

Load address + ff00

x = 0 -> LDH (a8), A
x = 1 -> LDH A, (a8)

## 1110 1000 - Add to stack pointer
Size 2, timing 16

ADD SP,r8

Add signed value at offset from program counter to SP

## 1111 1000 - Stack offset to memory address
Size 2, timing 12

LD HL, SP+r8

## 11xx 0001 - POP
Size 1, timing 12

xx = 16 bit register including Accumalator

## 11xx 0101 - PUSH
Size 1, timing 16

xx = 16 bit register including accumalator