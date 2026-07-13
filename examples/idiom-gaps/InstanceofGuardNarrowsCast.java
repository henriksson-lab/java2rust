class InstanceofGuardNarrowsCast {
    public boolean equals(Object obj) {
        if (!(obj instanceof InstanceofGuardNarrowsCast)) {
            return false;
        }
        return ((InstanceofGuardNarrowsCast) obj).dist().equals(dist());
    }

    Distribution dist() {
        return null;
    }
}

