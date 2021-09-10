| Value     | Controller | RAM | Battery | Timer | Rumble |
|-----------|------------|-----|---------|-------|--------|
| 0000 0000 | ROM        |     |         |       |        |
| 0000 0001 | MBC1       |     |         |       |        |
| 0000 0010 | MBC1       |  X  |         |       |        |
| 0000 0011 | MBC1       |  X  |    X    |       |        |
| 0000 0101 | MBC2       |  X  |         |       |        |
| 0000 0110 | MBC2       |  X  |    X    |       |        |
| 0000 1000 | ROM        |  X  |         |       |        |
| 0000 1001 | ROM        |  X  |    X    |       |        |
| 0000 1111 | MBC3       |  X  |    X    |   X   |        |
| 0001 0001 | MBC3       |     |         |       |        |
| 0001 0010 | MBC3       |  X  |         |       |        |
| 0001 0011 | MBC3       |  X  |    X    |       |        |
| 0001 1001 | MBC5       |     |         |       |        |
| 0001 1010 | MBC5       |  X  |         |       |        |
| 0001 1011 | MBC5       |  X  |    X    |       |        |
| 0001 1100 | MBC5       |     |         |       |    X   |
| 0001 1101 | MBC5       |  X  |         |       |    X   |
| 0001 1110 | MBC5       |  X  |    X    |       |    X   |



MBC1:

0x1-0x3: xxxx xxRB
R = RAM
B = Battery


MBC3:

0x10-0x13: xxxx TxRB
T = Timer
R = Rumble
B = Battery
