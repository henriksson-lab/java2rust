import java.util.Objects;

public final class NullableEnumFieldProbe {
    enum Shade { BLUE, RED }
    private final Shade shade;

    NullableEnumFieldProbe(Shade shade) {
        this.shade = Objects.requireNonNull(shade, "shade");
    }

    boolean selected() { return shade == Shade.BLUE; }
    boolean explicit() { return this.shade != Shade.RED; }

    public static int run() {
        NullableEnumFieldProbe blue = new NullableEnumFieldProbe(Shade.BLUE);
        NullableEnumFieldProbe red = new NullableEnumFieldProbe(Shade.RED);
        if (!blue.selected() || !blue.explicit()) return 1;
        if (red.selected() || red.explicit()) return 2;
        try {
            new NullableEnumFieldProbe(null);
            return 3;
        } catch (NullPointerException expected) {
            if (!"shade".equals(expected.getMessage())) return 4;
        }
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
