macro_rules! impl_deref_and_deref_mut {
    ($type:ty, $base_type:ty) => {
        impl Deref for $type {
            type Target = $base_type;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $type {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
}

macro_rules! impl_from_str_and_display {
    ($type:ty, $base_type:ty) => {
        impl FromStr for $type {
            type Err = ParseIntError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let value = s.parse::<$base_type>()?;
                Ok(Self(value))
            }
        }

        impl fmt::Display for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

macro_rules! impl_add_and_sub_with_base_type {
    ($type:ty, $base_type:ty) => {
        impl Add<$base_type> for $type {
            type Output = Self;

            fn add(self, other: $base_type) -> Self {
                Self(self.0 + other)
            }
        }

        impl Sub<$base_type> for $type {
            type Output = Self;

            fn sub(self, other: $base_type) -> Self {
                Self(self.0 - other)
            }
        }
    };
}
