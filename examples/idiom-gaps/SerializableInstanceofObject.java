import java.io.ObjectOutputStream;
import java.io.Serializable;

class SerializableInstanceofObject {
    void write(ObjectOutputStream out, Object value) throws Exception {
        if (value instanceof Serializable) {
            out.writeObject(value);
        } else {
            out.writeObject("not-serializable");
        }
    }

    void writeAll(ObjectOutputStream out, Object[] values) throws Exception {
        for (int i = 0; i < values.length; i++) {
            if (values[i] instanceof Serializable) {
                out.writeObject(values[i]);
            }
        }
    }
}
