public class OuterOnlyReceiver {
    int count;
    class Cursor {
        private int read() { return OuterOnlyReceiver.this.bump(); }
        int next() { return read(); }
    }
    int bump() { return ++count; }
    Cursor cursor() { return new Cursor(); }
    static Cursor escaping() { return new OuterOnlyReceiver().cursor(); }
    static int run() {
        OuterOnlyReceiver owner = new OuterOnlyReceiver();
        Cursor first = owner.cursor();
        Cursor second = owner.cursor();
        if (first.next() != 1) return 1;
        if (second.next() != 2) return 2;
        if (first.next() != 3 || owner.count != 3) return 3;
        Cursor escaped = escaping();
        if (escaped.next() != 1 || escaped.next() != 2) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
