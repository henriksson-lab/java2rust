class AnonymousTryCatchBase {
    AnonymousTryCatchBase() {}
    int value(int[] values) { return -1; }
}

interface AnonymousTryCatchReader {
    int value(int[] values);
    int fallback();
}

public class AnonymousTryCatchBodies {
    static AnonymousTryCatchBase makeClass() {
        return new AnonymousTryCatchBase() {
            int fallback = 7;

            @Override int value(int[] values) {
                try {
                    return values[0];
                } catch (ArrayIndexOutOfBoundsException ex) {
                    return this.fallback;
                }
            }
        };
    }

    static AnonymousTryCatchReader makeInterface() {
        return new AnonymousTryCatchReader() {
            int fallback = 9;

            public int value(int[] values) {
                try {
                    return values[0];
                } catch (ArrayIndexOutOfBoundsException ex) {
                    return this.fallback;
                }
            }

            public int fallback() { return this.fallback; }
        };
    }

    static int classValid() { return makeClass().value(new int[] { 3 }); }
    static int classCaught() { return makeClass().value(new int[] {}); }
    static int interfaceValid() { return makeInterface().value(new int[] { 5 }); }
    static int interfaceCaught() { return makeInterface().value(new int[] {}); }

    public static void main(String[] args) {
        System.out.println(classValid());
        System.out.println(classCaught());
        System.out.println(interfaceValid());
        System.out.println(interfaceCaught());
    }
}
