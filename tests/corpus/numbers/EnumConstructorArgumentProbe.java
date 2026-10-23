public class EnumConstructorArgumentProbe {
    enum Tag {
        A(7), B(9);
        final int code;
        Tag(int code) { this.code = code; }
    }
    static class Required {
        final Tag tag;
        final int number;
        Required(Tag tag, int a, int b, int c, int d) { this.tag=tag; number=a+b+c+d; }
        static Required make() { return new Required(Tag.A,1,2,3,4); }
    }
    static class Nullable {
        final Tag tag;
        Nullable(Tag tag) { this.tag=tag; }
    }
    public static int run() {
        Required value = Required.make();
        if (value.tag != Tag.A || value.tag.code != 7 || value.number != 10) return 1;
        Nullable absent = new Nullable(null);
        Nullable present = new Nullable(Tag.B);
        if (absent.tag != null || present.tag != Tag.B || present.tag.code != 9) return 2;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
