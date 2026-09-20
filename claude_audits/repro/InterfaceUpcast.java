// Java's interface inheritance permits this conversion without allocating or
// changing the object's identity. The Rust lowering must convert the owned
// `Box<dyn Combined>` carrier into an owned `Box<dyn Left>` carrier while
// preserving the same anonymous implementation.
interface Left {
    int shared();
}

interface Right {
    int shared();
}

interface Combined extends Left, Right {
}

public class InterfaceUpcast {
    static Combined make() {
        return new Combined() {
            public int shared() { return 42; }
        };
    }

    static int take(Left value) {
        return value.shared();
    }

    public static int run() {
        return take(make());
    }
}
