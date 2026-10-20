interface DrawContract {
    int next();
    byte[] state();
    void restore(byte[] state);
}
interface RestoredDrawContract extends DrawContract {
    int bounded(int bound);
}
abstract class DrawDeclaration implements RestoredDrawContract {
    static int checkBound(int bound) {
        if (bound <= 0) throw new IllegalArgumentException("bound");
        return bound;
    }
    public int bounded(int bound) { return next() % checkBound(bound); }
    protected int doubled() { return next() * 2; }
}
abstract class LongDrawDeclaration extends DrawDeclaration {
    abstract int advance();
    public int next() { return advance(); }
    static void fillInto(DrawContract source, byte[] out) {
        fillRange(source, out, 0, out.length);
    }
    static void fillRange(DrawContract source, byte[] out, int start, int length) {
        for (int i = start; i < start + length; i++) out[i] = (byte) source.next();
    }
    void fillBytes(byte[] out) { fillInto(this, out); }
    int decorated() { return doubled() + 1; }
}
class StatefulDraw extends LongDrawDeclaration {
    int value = 2;
    int advance() { return ++value; }
    public byte[] state() { return new byte[] { (byte) value }; }
    public void restore(byte[] state) { value = state[0]; }
    protected int doubled() { return super.doubled() + 10; }
}
class StrideDraw extends LongDrawDeclaration {
    int value = 5;
    int advance() { value += 2; return value; }
    public byte[] state() { return new byte[] { (byte) value }; }
    public void restore(byte[] state) { value = state[0]; }
}
class AbstractDeclarationContracts {
    static int run() {
        StatefulDraw counter = new StatefulDraw();
        RestoredDrawContract view = counter;
        if (view.next() != 3 || counter.value != 3) return 1;
        byte[] saved = view.state();
        if (view.next() != 4) return 2;
        view.restore(saved);
        if (counter.value != 3) return 3;
        byte[] out = new byte[3];
        counter.fillBytes(out);
        if (out[0] != 4 || out[1] != 5 || out[2] != 6 || counter.value != 6) return 4;
        if (view.bounded(5) != 2 || counter.value != 7) return 5;
        if (counter.decorated() != 27 || counter.value != 8) return 6;
        try { view.bounded(0); return 7; }
        catch (IllegalArgumentException error) {}
        StrideDraw stride = new StrideDraw();
        DrawContract other = stride;
        if (other.next() != 7 || stride.value != 7) return 8;
        stride.fillBytes(out);
        if (out[0] != 9 || out[1] != 11 || out[2] != 13 || stride.value != 13) return 9;
        return 0;
    }
}
