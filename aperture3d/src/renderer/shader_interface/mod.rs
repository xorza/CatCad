//! What the WGSL declares, read back so the Rust that feeds it is held to it.

use wgpu::naga;

/// The shader module as naga parses it, kept for the one question wgpu does
/// not ask: whether a layout written in Rust says exactly what the shader
/// declares.
///
/// wgpu checks each vertex input the shader declares against the buffer
/// layout, but by base type alone — a `vec3<f32>` read out of a `Float32x2`
/// passes — and it does not check the other way: an attribute bound that no
/// input declares is fetched for nothing, and only a Vulkan validation layer
/// says so. A uniform struct it checks by total size. Everything past that was
/// two lists kept in step by hand, so this reads the shader's side of each one
/// back and holds the Rust side to it, whole.
///
/// Built once with the shader module, and every check runs where the pairing
/// is made — [`Pipelines::build`](super::pipelines::Pipelines::build) and the
/// uniform buffer beside it — so no pipeline can be built outside it and there
/// is no list of pipelines here to fall behind. Release builds check too: it is
/// one parse at startup, under a millisecond for the whole module, and a layout
/// that disagrees draws out of the wrong bytes in release exactly as it does in
/// debug.
#[derive(Debug)]
pub(super) struct ShaderInterface {
    module: naga::Module,
}

impl ShaderInterface {
    /// Read `source` the way wgpu is about to.
    pub(super) fn parse(source: &str) -> Self {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
        Self { module }
    }

    /// Panics unless the vertex entry point `entry` declares exactly the
    /// locations `attributes` binds, each as the type its format is read as.
    ///
    /// Both ways round. An input with no attribute is wgpu's to refuse; an
    /// attribute with no input is a record shipping bytes nothing reads, and a
    /// count that differs is a field read short or read into the next one.
    pub(super) fn hold_vertex_inputs(&self, entry: &str, attributes: &[wgpu::VertexAttribute]) {
        let point = self
            .module
            .entry_points
            .iter()
            .find(|point| point.stage == naga::ShaderStage::Vertex && point.name == entry)
            .unwrap_or_else(|| panic!("no vertex entry point `{entry}`"));

        let mut declared = Vec::new();
        for argument in &point.function.arguments {
            match &argument.binding {
                Some(naga::Binding::Location { location, .. }) => {
                    declared.push(Input {
                        location: *location,
                        ty: argument.ty,
                    });
                }
                Some(naga::Binding::BuiltIn(_)) => {}
                None => {
                    let naga::TypeInner::Struct { members, .. } =
                        &self.module.types[argument.ty].inner
                    else {
                        unreachable!("an unbound entry-point argument is a struct");
                    };
                    for member in members {
                        if let Some(naga::Binding::Location { location, .. }) = member.binding {
                            declared.push(Input {
                                location,
                                ty: member.ty,
                            });
                        }
                    }
                }
            }
        }
        declared.sort_by_key(|input| input.location);

        let mut bound: Vec<_> = attributes.iter().collect();
        bound.sort_by_key(|attribute| attribute.shader_location);
        let declared_at: Vec<_> = declared.iter().map(|input| input.location).collect();
        let bound_at: Vec<_> = bound
            .iter()
            .map(|attribute| attribute.shader_location)
            .collect();
        assert_eq!(
            declared_at, bound_at,
            "`{entry}` declares the first locations and its record binds the second"
        );
        for (input, attribute) in declared.iter().zip(bound) {
            let read_as = Self::read_as(attribute.format);
            let inner = &self.module.types[input.ty].inner;
            assert!(
                *inner == read_as,
                "`{entry}` declares location {} as {inner:?}, and its record binds \
                 {:?}, which is read as {read_as:?}",
                input.location,
                attribute.format,
            );
        }
    }

    /// Panics unless the WGSL struct `name` has exactly the members `members`
    /// names, each at the offset given, and spans `size` bytes.
    ///
    /// Offsets and the total rather than each member's size: a member of the
    /// wrong width moves every member after it, and the last one moves the
    /// total.
    pub(super) fn hold_struct_layout(&self, name: &str, members: &[StructMember<'_>], size: usize) {
        let (_, declared) = self
            .module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("no struct `{name}` in the shader"));
        let naga::TypeInner::Struct {
            members: declared,
            span,
        } = &declared.inner
        else {
            panic!("`{name}` in the shader is not a struct");
        };
        let declared: Vec<_> = declared
            .iter()
            .map(|member| StructMember {
                name: member.name.as_deref().unwrap_or_default(),
                offset: member.offset as usize,
            })
            .collect();
        assert_eq!(
            declared, members,
            "`{name}` in the shader is laid out as the first, and in Rust as the second"
        );
        assert_eq!(
            *span as usize, size,
            "`{name}` spans {span} bytes in the shader and {size} in Rust"
        );
    }

    /// The shader type a vertex format arrives as.
    ///
    /// Only the formats this crate binds; one more is a line here, and until it
    /// is written the check refuses it rather than guessing.
    fn read_as(format: wgpu::VertexFormat) -> naga::TypeInner {
        use naga::{Scalar, TypeInner, VectorSize};
        use wgpu::VertexFormat as F;
        let vector = |size, scalar| TypeInner::Vector { size, scalar };
        match format {
            F::Float32 => TypeInner::Scalar(Scalar::F32),
            F::Float32x2 => vector(VectorSize::Bi, Scalar::F32),
            F::Float32x3 => vector(VectorSize::Tri, Scalar::F32),
            F::Float32x4 => vector(VectorSize::Quad, Scalar::F32),
            F::Uint32 => TypeInner::Scalar(Scalar::U32),
            F::Sint32 => TypeInner::Scalar(Scalar::I32),
            other => panic!("{other:?} is not a format the interface check reads yet"),
        }
    }
}

/// One member of a struct shared with the shader, as both sides lay it out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StructMember<'a> {
    pub(super) name: &'a str,
    pub(super) offset: usize,
}

/// One located vertex input an entry point declares.
#[derive(Debug, Clone, Copy)]
struct Input {
    location: u32,
    ty: naga::Handle<naga::Type>,
}

#[cfg(test)]
mod tests;
