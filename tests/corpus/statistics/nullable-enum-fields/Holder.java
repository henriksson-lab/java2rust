package neutral.palette;

import java.util.Objects;

public final class Holder {
    private final Palette shade;
    public Holder(Palette shade) {
        this.shade = Objects.requireNonNull(shade, "shade");
    }
    public boolean selected() { return shade == Palette.BLUE; }
    public boolean explicit() { return this.shade != Palette.RED; }
}
