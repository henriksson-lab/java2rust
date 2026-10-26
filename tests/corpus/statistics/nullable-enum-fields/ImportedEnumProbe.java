package neutral.client;

import neutral.palette.Holder;
import neutral.palette.Palette;

public final class ImportedEnumProbe {
    public static int run() {
        Holder blue = new Holder(Palette.BLUE);
        Holder red = new Holder(Palette.RED);
        if (!blue.selected() || !blue.explicit()) return 1;
        if (red.selected() || red.explicit()) return 2;
        try {
            new Holder(null);
            return 3;
        } catch (NullPointerException expected) {
            if (!"shade".equals(expected.getMessage())) return 4;
        }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
