# Understanding Filler Generation and Onion Building

A beginner-friendly guide to Lightning Network's Sphinx packet construction.

## Table of Contents

1. [The Big Picture: What Are We Building?](#the-big-picture-what-are-we-building)
2. [A Simple Analogy: Nested Envelopes](#a-simple-analogy-nested-envelopes)
3. [The Problem Filler Solves](#the-problem-filler-solves)
4. [Cryptographic Building Blocks](#cryptographic-building-blocks)
5. [Filler Generation Explained](#filler-generation-explained)
6. [Onion Building Step by Step](#onion-building-step-by-step)
7. [Code Walkthrough](#code-walkthrough)

---

## The Big Picture: What Are We Building?

Imagine you want to send a payment through the Lightning Network from yourself (Alice) to a friend (Dave) through two intermediaries (Bob and Carol):

```
Alice -> Bob -> Carol -> Dave
```

The challenge: **How do we route this payment so that:**
- Bob only knows "I received this from Alice, I should send it to Carol"
- Carol only knows "I received this from Bob, I should send it to Dave"
- Nobody can figure out the full path or their position in it

This is called **onion routing** - like layers of an onion, each hop can only "peel" their own layer.

---

## A Simple Analogy: Nested Envelopes

Think of it like sending nested physical envelopes:

```
[ Bob's Envelope                                    ]
[   "Dear Bob, please forward to Carol"             ]
[   [ Carol's Envelope                         ]    ]
[   [   "Dear Carol, please forward to Dave"   ]    ]
[   [   [ Dave's Envelope                 ]    ]    ]
[   [   [   "Dear Dave, here's payment!"  ]    ]    ]
```

- Bob opens his envelope, reads his instructions, and forwards the inner envelope to Carol
- Carol does the same
- Dave finally receives the payment

**But there's a problem!** Each time someone opens an envelope, the package gets smaller. If Bob receives a thick envelope and forwards a thin one, Carol knows she's not the first hop. She can count how many layers remain!

**Solution: Filler!** We pad each envelope so it's always the same size after opening.

---

## The Problem Filler Solves

In the Lightning Network, the onion packet is always exactly **1300 bytes** (plus header and HMAC).

### Without Filler

```
Initial packet (1300 bytes):
[Payload1][Payload2][Payload3][zeros zeros zeros zeros...]

After Bob processes (shifts left):
[Payload2][Payload3][zeros zeros zeros zeros zeros zeros...]
                                        ^^^^ MORE ZEROS!
```

Bob could tell Carol: "Hey, I removed 65 bytes from the front, now there are more zeros at the end. Count them and you'll know you're hop #2!"

### With Filler

```
Initial packet (1300 bytes):
[Payload1][Payload2][Payload3][encrypted filler data...]

After Bob processes (shifts left + decrypts):
[Payload2][Payload3][different encrypted filler data...]
```

The filler is carefully constructed so that when each hop decrypts, they see random-looking data at the end - not revealing zeros that would expose the route length.

---

## Cryptographic Building Blocks

Before diving into filler generation, let's understand the tools we use.

### XOR: The Magic Reversible Operation

XOR (exclusive or) is like a light switch that can be flipped twice to return to the original state:

```
Original:    10110011
XOR with:    11001010
Result:      01111001  (encrypted)

Encrypted:   01111001
XOR with:    11001010  (same key)
Result:      10110011  (back to original!)
```

**Key insight:** `A XOR B XOR B = A`

In Rust, XOR is the `^` operator:

```rust
let original: u8 = 0b10110011;
let key: u8 = 0b11001010;

let encrypted = original ^ key;  // 0b01111001
let decrypted = encrypted ^ key; // 0b10110011 (original!)
```

### Shared Secrets with ECDH

Alice needs to create a secret with each hop **without** talking to them beforehand. She uses Elliptic Curve Diffie-Hellman (ECDH):

```
Alice has: private key 'a', public key 'A' (A = a * G)
Bob has:   private key 'b', public key 'B' (B = b * G)

Alice computes: a * B = a * b * G = shared_secret
Bob computes:   b * A = b * a * G = shared_secret

They get the SAME secret without exchanging private keys!
```

### Key Derivation

From each shared secret, we derive multiple keys for different purposes:

```rust
// From the code:
fn derive_key(shared_secret: &[u8; 32], key_type: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key_type).unwrap();
    mac.update(shared_secret);
    mac.finalize().into_bytes().into()
}

// Usage:
let rho_key = derive_key(&shared_secret, b"rho");  // for encryption
let mu_key = derive_key(&shared_secret, b"mu");    // for HMAC
let pad_key = derive_key(&shared_secret, b"pad");  // for initial padding
```

- **rho**: Generates the byte stream for XOR encryption/decryption
- **mu**: Creates HMAC to verify packet integrity
- **pad**: Creates random-looking initial padding

### HMAC: Digital Tamper Seal

HMAC is like a tamper-evident seal on a package. If anyone modifies the contents, the seal breaks.

```rust
// Computing an HMAC
let mut mac = HmacSha256::new_from_slice(&mu_key).unwrap();
mac.update(&routing_info);          // The data we're protecting
mac.update(&associated_data);       // Extra context
let hmac = mac.finalize().into_bytes();
```

Each hop checks: "Does this HMAC match? If not, the packet was tampered with!"

---

## Filler Generation Explained

Now for the tricky part. Let's build intuition step by step.

### The Core Idea

Remember: each hop will:
1. Decrypt the packet using XOR with a pseudo-random stream
2. Shift everything left (removing their payload from the front)
3. See "random-looking" data at the end (not zeros!)

Alice needs to **pre-compute** what each hop will see at the end. She does this by simulating the encryption/decryption process.

### A Simple Example with 3 Hops

Let's say we have 3 hops with payload sizes: 65, 65, and 65 bytes.
Each payload + HMAC = 65 + 32 = 97 bytes.

```
Packet size: 1300 bytes
After Hop 1 processes: bytes 0-96 removed, 97-1299 shift left
After Hop 2 processes: bytes 0-96 removed, 97-1299 shift left
```

**The filler region is the "extra" space at the end that each hop creates when shifting.**

### Visual Walkthrough

```
BEFORE HOP 1 PROCESSES:
Position: [0........97........194........291.................1300]
Content:  [Payload1+HMAC][Payload2+HMAC][Payload3+HMAC][filler...]

HOP 1 DECRYPTS (XOR with rho stream):
- Entire 1300 bytes XORed with Hop1's rho stream
- Hop1 reads their payload (bytes 0-96)

HOP 1 SHIFTS LEFT:
Position: [0........97........194.................1203.......1300]
Content:  [Payload2+HMAC][Payload3+HMAC][filler..........][????]
                                                           ^^^
                                          97 bytes of "new space"
```

That "new space" (bytes 1203-1299) would be zeros if we didn't have filler. Hop 2 could count zeros to learn the route length!

### The Filler Construction Algorithm

Alice simulates what happens:

```
For each hop (except the last):
    1. Generate the same rho keystream that hop will use
    2. Calculate which bytes will become "new space" after shifting
    3. XOR those bytes into the accumulated filler
```

In the code:

```rust
for i in 0..(num_hops - 1) {
    // Generate the 2600-byte keystream (we need extra for filler calculation)
    let mut keystream = vec![0u8; 2600];
    let mut cipher = ChaCha20::new(rho_keys[i].as_ref().into(), &[0u8; 12].into());
    cipher.apply_keystream(&mut keystream);

    // Calculate where filler bytes come from in the extended stream
    let filler_start = 1300 - cumulative_sizes[i];
    let filler_end = 1300 + json_data.hops[i].payload.len();
    let filler_length = filler_end + 32 - filler_start;

    // XOR zeros with keystream at those positions
    let mut this_filler = vec![0u8; filler_length];
    for j in filler_start..(filler_end + 32) {
        let idx = j - filler_start;
        this_filler[idx] ^= keystream[j];
    }

    // Accumulate with previous filler
    // (because previous fillers also get XORed by this hop's stream)
    // ...
}
```

### Why 2600 Bytes?

When a hop processes the packet, they conceptually work with **1300 original bytes + 1300 bytes of zeros** (the "overflow" area). The filler comes from XORing the keystream in positions 1300-2600 with what would otherwise be zeros.

```
Position:   [0.......................1300.......................2600]
Keystream:  [encrypt packet region...][filler source region........]
```

---

## Onion Building Step by Step

Now let's build the actual onion packet. We work **backwards** from the last hop to the first.

### Why Backwards?

Think about it: when Bob decrypts, he'll see Carol's payload. So we need to:
1. First encrypt Dave's payload (innermost)
2. Then wrap it with Carol's layer
3. Finally wrap everything with Bob's layer

It's like sealing envelopes from inside out.

### The Algorithm

```
1. Start with random-looking padding (1300 bytes)
2. For each hop, from LAST to FIRST:
   a. Shift everything right to make room
   b. Insert this hop's payload + HMAC at the front
   c. XOR encrypt the whole thing with this hop's rho stream
   d. If this is the last hop, insert the filler at the end
   e. Compute the HMAC for the next iteration
```

### Code Walkthrough

```rust
// 1. Initialize with pseudo-random padding
let pad_key = derive_key(&json_data.session_key, b"pad");
let mut routing_info = [0u8; 1300];
let mut pad_cipher = ChaCha20::new(pad_key.as_ref().into(), &[0u8; 12].into());
pad_cipher.apply_keystream(&mut routing_info);

let mut current_hmac = [0u8; 32];  // Start with zeros (final recipient doesn't need HMAC)

// 2. Process hops in REVERSE order
for i in (0..num_hops).rev() {
    let payload = &json_data.hops[i].payload;
    let shift_amount = payload.len() + 32;

    // 3. Shift right - make room at the front
    routing_info.copy_within(0..1300 - shift_amount, shift_amount);

    // 4. Insert payload at position 0
    routing_info[0..payload.len()].copy_from_slice(payload);

    // 5. Insert current HMAC after payload
    routing_info[payload.len()..payload.len() + 32].copy_from_slice(&current_hmac);

    // 6. Encrypt with ChaCha20 (XOR with rho stream)
    for j in 0..1300 {
        routing_info[j] ^= rho_streams[i][j];
    }

    // 7. Insert filler - only on last hop (first iteration when going backwards)
    if i == num_hops - 1 {
        let filler_start = 1300 - filler.len();
        routing_info[filler_start..].copy_from_slice(&filler);
    }

    // 8. Compute HMAC for the next outer layer
    let mut mac = HmacSha256::new_from_slice(&mu_keys[i]).unwrap();
    mac.update(&routing_info);
    mac.update(&json_data.associated_data);
    current_hmac = mac.finalize().into_bytes().into();
}
```

### Visual Example: 2-Hop Build

```
INITIAL STATE (random padding):
[random random random random random random random random...]

ITERATION 1 (i=1, Carol - the last hop):
After shift right:    [empty space......][random random random...]
After insert payload: [Carol's Payload][HMAC=0][random random....]
After XOR encrypt:    [encrypted encrypted encrypted encrypted...]
After insert filler:  [encrypted encrypted...][FILLER BYTES HERE]
Compute HMAC for Bob

ITERATION 2 (i=0, Bob - the first hop):
After shift right:    [empty space......][Carol's encrypted layer...]
After insert payload: [Bob's Payload][Bob's HMAC][Carol's layer...]
After XOR encrypt:    [encrypted encrypted encrypted encrypted...]
(No filler insert - Bob isn't the last hop)
Compute final HMAC
```

---

## Code Walkthrough

Let's trace through the actual Rust code section by section.

### Input Parsing

```rust
#[serde_as]
#[derive(Debug, serde::Deserialize)]
struct Input {
    #[serde_as(as = "Hex")]
    session_key: [u8; 32],        // Alice's secret for this payment
    #[serde_as(as = "Hex")]
    associated_data: [u8; 32],    // Extra data included in HMACs
    hops: Vec<Hop>,               // List of hops in the route
}

#[serde_as]
#[derive(Debug, serde::Deserialize)]
struct Hop {
    #[serde_as(as = "Hex")]
    pubkey: [u8; 33],             // This hop's public key (compressed)
    #[serde_as(as = "Hex")]
    payload: Vec<u8>,             // Instructions for this hop
}
```

The `#[serde_as(as = "Hex")]` attribute tells Rust to parse hex strings from JSON into byte arrays.

### Generating Shared Secrets

```rust
// Start with the session key
let secret_key = SecretKey::from_byte_array(json_data.session_key)?;
let public_key = PublicKey::from_secret_key(SECP256K1, &secret_key).serialize();

let mut shared_secrets = Vec::new();
let mut ephemeral_privkey = secret_key;
let mut ephemeral_pubkey = public_key;

for hop in json_data.hops.iter() {
    let hop_publickey = PublicKey::from_slice(&hop.pubkey)?;

    // ECDH: multiply hop's public key by our ephemeral private key
    let scalar = Scalar::from(ephemeral_privkey);
    let shared_point = hop_publickey.mul_tweak(SECP256K1, &scalar)?;

    // Hash the shared point to get the shared secret
    let shared_bytes = shared_point.serialize();
    let shared_secret: [u8; 32] = Sha256::digest(&shared_bytes).into();
    shared_secrets.push(shared_secret);

    // IMPORTANT: Blind the ephemeral key for the next hop
    // This is so each hop can re-derive everything from what they receive
    let mut blinding_input = Vec::new();
    blinding_input.extend_from_slice(&ephemeral_pubkey);
    blinding_input.extend_from_slice(&shared_secret);
    let blinding_factor: [u8; 32] = Sha256::digest(blinding_input).into();

    let blinding_scalar = SecretKey::from_byte_array(blinding_factor)?;
    ephemeral_privkey = ephemeral_privkey.mul_tweak(&blinding_scalar.into())?;
    ephemeral_pubkey = PublicKey::from_secret_key(SECP256K1, &ephemeral_privkey).serialize();
}
```

**Key insight: Blinding** - Each hop receives the same ephemeral public key, but Alice "blinds" her private key at each step. This allows the next hop to compute the same shared secret without needing the original private key.

### Filler Generation (Detailed)

```rust
// Calculate how much space each hop takes: payload + 32-byte HMAC
let hop_sizes: Vec<usize> = json_data.hops.iter()
    .map(|h| h.payload.len() + 32)
    .collect();

// Cumulative sizes: how much total space all previous hops take
let mut cumulative_sizes: Vec<usize> = Vec::with_capacity(num_hops);
for i in 0..num_hops {
    cumulative_sizes.push(hop_sizes.iter().take(i).sum());
}
// Example: [0, 97, 194] for three 65-byte payloads

let mut filler_accumulators: Vec<Vec<u8>> = Vec::with_capacity(num_hops);

for i in 0..(num_hops - 1) {
    // Generate 2600-byte keystream for this hop
    let mut keystream = vec![0u8; 2600];
    let mut cipher = ChaCha20::new(rho_keys[i].as_ref().into(), &[0u8; 12].into());
    cipher.apply_keystream(&mut keystream);

    // The filler region is where "new zeros" would appear after this hop shifts
    // filler_start: where in the extended stream the filler begins
    // filler_end: where it ends
    let filler_start = 1300 - cumulative_sizes[i];
    let filler_end = 1300 + json_data.hops[i].payload.len();
    let filler_length = filler_end + 32 - filler_start;

    // XOR zeros with keystream at filler position
    let mut this_filler = vec![0u8; filler_length];
    for j in filler_start..(filler_end + 32) {
        let idx = j - filler_start;
        this_filler[idx] ^= keystream[j];
    }

    // Accumulate: each hop's filler XORs with all previous fillers
    if i == 0 {
        filler_accumulators.push(this_filler);
    } else {
        let prev_filler = &filler_accumulators[i - 1];
        let mut filler_acc = this_filler;
        for j in 0..cumulative_sizes[i] {
            filler_acc[j] ^= prev_filler[j];
        }
        filler_accumulators.push(filler_acc);
    }
}
```

### Final Onion Assembly

```rust
// Assemble the final packet: version + pubkey + routing_info + hmac
let mut onion = Vec::with_capacity(1366);
onion.push(0x00);                        // Version byte (always 0)
onion.extend_from_slice(&public_key);    // 33-byte compressed public key
onion.extend_from_slice(&routing_info);  // 1300-byte encrypted payload
onion.extend_from_slice(&current_hmac);  // 32-byte HMAC

// Total: 1 + 33 + 1300 + 32 = 1366 bytes
```

---

## Summary

1. **Problem**: Onion routing needs fixed-size packets so nobody can guess route length
2. **Solution**: Filler bytes make "empty space" look random after each hop decrypts
3. **Building backwards**: We encrypt from innermost (last hop) to outermost (first hop)
4. **XOR is key**: Same operation encrypts and decrypts - applying it twice gets the original
5. **Shared secrets**: ECDH lets Alice create secrets with each hop without prior communication

---

## Further Reading

- [BOLT #4: Onion Routing Protocol](https://github.com/lightning/bolts/blob/master/04-onion-routing.md) - The official specification
- [Elle Mouton's Sphinx Tutorial](https://ellemouton.com/posts/sphinx/) - Excellent visual walkthrough
- [Mastering the Lightning Network, Chapter 10](https://www.oreilly.com/library/view/mastering-the-lightning/9781492054856/ch10.html) - Comprehensive book chapter
