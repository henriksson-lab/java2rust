package examples.objects;

public class EnumConstantsWithFields {
    enum Mode {
        FAST(1),
        SLOW(2);

        private final int id;

        Mode(int id) {
            this.id = id;
        }

        int id() {
            return id;
        }
    }

    public int id(Mode mode) {
        return mode.id();
    }
}
