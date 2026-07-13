class ByteArrayGetterDispatchCoercion {
    byte[] bases(Read read) {
        byte[] value = read.getBasesNoCopy();
        return value;
    }
}

