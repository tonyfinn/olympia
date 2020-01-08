# olympia_derive

olympia_derive currently provides one derive macro, OlympiaInstruction.

A usage example for a two argument instruction is below:

```rust
#[derive(OlympiaInstruction)]
#[olympia(
    opcode=0x00AA_A111,
    label="LD", 
    excluded(0b1010_1100)
)]
struct LoadRegisterConstant8 {
    #[olympia(dest, mask=0xA)]
    dest: ByteRegisterLookup,
    #[olympia(src)]
    src: u8,
}
```

A usage example for one argument instruction is below:


```rust
#[derive(OlympiaInstruction)]
#[olympia(
    opcode=0x110A_A000, 
    label="RET", 
)]
struct ReturnIf {
    #[olympia(single, mask=0xA)]
    dest: ByteRegisterLookup,
}
```


A usage example for no argument instruction is below:


```rust
#[derive(OlympiaInstruction)]
#[olympia(
    opcode=0x1100_1001, 
    label="RET", 
)]
struct ReturnIf;
```

