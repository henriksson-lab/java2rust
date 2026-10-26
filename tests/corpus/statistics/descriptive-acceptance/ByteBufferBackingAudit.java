package bufferbacking;
class ByteBuffer { static int allocate(int value) { return value + 40; } }
class BufferHolder {
    java.nio.ByteBuffer buffer = java.nio.ByteBuffer.allocate(4);
    byte[] initialized = buffer.array();
    byte[] passed = pass(buffer.array());
    static byte[] pass(byte[] value) { value[0] = 77; return value; }
    byte[] supplied;
    BufferHolder(byte[] value) { supplied = value; }
    int check() { return initialized == buffer.array() && passed == initialized && initialized[0] == 77 ? 0 : 1; }
}
public class ByteBufferBackingAudit {
    static java.nio.ByteBuffer stored = java.nio.ByteBuffer.allocate(4);
    static byte[] storedBytes = stored.array();
    static byte[] view(java.nio.ByteBuffer value) { return value.array(); }
    static byte[] echo(byte[] value) { value[0] = 99; return value; }
    static byte[] chainedEcho(byte[] value) { value[0] = 88; return value; }
    public static int run() {
        if (ByteBuffer.allocate(2) != 42) return 1;
        java.nio.ByteBuffer buffer = java.nio.ByteBuffer.allocate(16);
        java.nio.ByteBuffer alias = buffer;
        if (alias != buffer) return 12;
        byte[] first = buffer.array();
        byte[] second = view(alias);
        if (first != second || first.length != 16) return 2;
        alias.putInt(0x01020304);
        if (buffer.position() != 4 || first[0] != 1 || first[3] != 4) return 3;
        first[4] = 9;
        if (buffer.array()[4] != 9) return 4;
        buffer.putLong(0x0102030405060708L);
        if (alias.position() != 12 || alias.remaining() != 4 || !alias.hasRemaining()) return 5;
        if (second[4] != 1 || second[11] != 8) return 6;
        try { buffer.putLong(7); return 7; }
        catch (java.nio.BufferOverflowException expected) { if (expected.getMessage() != null) return 16; }
        if (buffer.position() != 12 || first[12] != 0 || first[15] != 0) return 8;
        alias.putInt(-1);
        if (buffer.hasRemaining() || second[15] != -1) return 9;
        try { java.nio.ByteBuffer.allocate(-1); return 10; }
        catch (IllegalArgumentException expected) { }
        stored.putInt(0x11223344);
        if (storedBytes[0] != 17 || storedBytes[3] != 68 || stored.array() != storedBytes) return 11;
        java.nio.ByteBuffer third = java.nio.ByteBuffer.allocate(4);
        byte[] echoed = echo(third.array());
        BufferHolder holder = new BufferHolder(third.array());
        if (holder.check() != 0 || holder.supplied != echoed || third.array()[0] != 99) return 13;
        holder.supplied[1] = 98;
        if (echoed[1] != 98) return 14;
        byte[] chained = chainedEcho(java.nio.ByteBuffer.allocate(4).putInt(0).array());
        if (chained[0] != 88) return 15;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
