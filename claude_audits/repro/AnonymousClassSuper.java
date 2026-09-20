// An anonymous class is a subtype of Base and must retain both inherited state
// and a qualified superclass call. A local Rust struct cannot be returned as a
// concrete Base without the declared-type carrier described by audits 0002/2b.
class Base {
    final int offset;

    Base(int offset) {
        this.offset = offset;
    }

    int value(int input) {
        return input + offset;
    }
}

public class AnonymousClassSuper {
    static Base make() {
        return new Base(3) {
            @Override
            int value(int input) {
                return super.value(input) * 2;
            }
        };
    }

    public static int run() {
        return make().value(4);
    }
}
