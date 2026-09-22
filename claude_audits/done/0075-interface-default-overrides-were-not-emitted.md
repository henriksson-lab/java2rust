# 0075 — Concrete overrides of interface defaults were not emitted

Status: **fixed** — a class method that overrides an interface default is now
emitted in the class's trait implementation, while an inherited default still
uses the trait body.

## Root cause

`trait_impl_for_type` enumerated only abstract interface contracts. That was
enough to require abstract implementations, but it excluded every default
method before the existing override detector could see it. A class that
declared its own implementation therefore received an empty trait impl and
silently dispatched to the interface default.

Interface instance-contract discovery now has an explicit mode that includes
defaults. Trait implementation lowering uses that complete set; the existing
method selector emits a forwarding method only when the concrete class really
overrides the default.

## Verification

`interface_default_method_body_is_emitted_and_inherited` checks both sides of
the contract in generated Rust: inheritance retains the default body, and a
concrete override wins at runtime.
