use std::{
    fmt,
    marker::PhantomData,
    ops::{Index, IndexMut},
};

/// Key-Typed Vec-backed data structure
pub struct Map<K, V>(Vec<V>, PhantomData<K>);

impl<K, V> Map<K, V>
where
    K: From<usize> + 'static,
{
    pub fn new() -> Self {
        Self(Vec::new(), PhantomData)
    }

    pub fn push(&mut self, value: V) -> K {
        let index = self.0.len();
        self.0.push(value);
        K::from(index)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> + Clone + 'static {
        (0..self.0.len()).map(K::from)
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.0.iter()
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.keys().zip(self.0.iter())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
impl<K, V> Map<K, V>
where
    K: Into<usize> + 'static,
{
    pub fn get_many_mut<const N: usize>(
        &mut self,
        indices: [K; N],
    ) -> Result<[&mut V; N], std::slice::GetDisjointMutError> {
        let indices: [usize; N] = indices.map(K::into);
        self.0.get_disjoint_mut(indices)
    }
}

impl<K, V> Default for Map<K, V>
where
    K: From<usize> + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Index<K> for Map<K, V>
where
    K: Into<usize>,
{
    type Output = V;

    fn index(&self, key: K) -> &Self::Output {
        &self.0[key.into()]
    }
}

impl<K, V> IndexMut<K> for Map<K, V>
where
    K: Into<usize>,
{
    fn index_mut(&mut self, key: K) -> &mut Self::Output {
        &mut self.0[key.into()]
    }
}

impl<K, V> FromIterator<V> for Map<K, V> {
    fn from_iter<T: IntoIterator<Item = V>>(iter: T) -> Self {
        Map(iter.into_iter().collect(), PhantomData)
    }
}

impl<K: fmt::Display, V: fmt::Display> fmt::Display for Map<K, V>
where
    K: From<usize> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "{{}}");
        }

        writeln!(f, "{{")?;

        for (k, v) in self.iter() {
            writeln!(f, "  {k} -> {v}")?;
        }

        writeln!(f, "}}")
    }
}

impl<K: fmt::Debug, V: fmt::Debug> fmt::Debug for Map<K, V>
where
    K: From<usize> + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "{{}}");
        }

        writeln!(f, "{{")?;

        for (k, v) in self.iter() {
            writeln!(f, "  {k:?} -> {v:?}")?;
        }

        writeln!(f, "}}")
    }
}

#[macro_export]
macro_rules! new_vec_key_impls {
    ($name:ident, $fmt:literal) => {
        impl From<usize> for $name {
            fn from(i: usize) -> $name {
                $name(i)
            }
        }

        impl Into<usize> for $name {
            fn into(self) -> usize {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}.{}", $fmt, self.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "{}.{}", $fmt, self.0)
            }
        }
    };
}

#[macro_export]
macro_rules! new_vec_key {
    (struct $name:ident, $fmt:literal) => {
        #[derive(PartialEq, Eq, Hash, Clone, Copy)]
        struct $name(usize);
        new_vec_key_impls!($name, $fmt);
    };

    (pub struct $name:ident, $fmt:literal) => {
        #[derive(PartialEq, Eq, Hash, Clone, Copy)]
        pub struct $name(usize);
        new_vec_key_impls!($name, $fmt);
    };
}
