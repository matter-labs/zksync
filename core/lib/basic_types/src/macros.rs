macro_rules! basic_type {
    ($(#[$attr:meta])* $name:ident, $type:ty) => {
        $(#[$attr])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd, Ord, Default
        )]
        pub struct $name(pub $type);

        impl Deref for $name {
            type Target = $type;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl From<$type> for $name {
            fn from(value: $type) -> Self {
                Self(value)
            }
        }

        impl FromStr for $name {
            type Err = ParseIntError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let value = s.parse::<$type>()?;
                Ok(Self(value))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl Add<$type> for $name {
            type Output = Self;

            fn add(self, other: $type) -> Self {
                Self(self.0 + other)
            }
        }

        impl Sub<$type> for $name {
            type Output = Self;

            fn sub(self, other: $type) -> Self {
                Self(self.0 - other)
            }
        }
    };
}
