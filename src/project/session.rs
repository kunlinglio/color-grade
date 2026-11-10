use std::path::PathBuf;
use std::sync::Arc;

use crate::project::compiler::Compiler;
use crate::render::{BlendMode, Frame, RenderList};

/// A opened or edited media inside a project.
pub struct Session {
    workflow: WorkFlow,
    decoded: Arc<Frame>,
}

impl Session {
    pub fn new(path: PathBuf) -> anyhow::Result<Self> {
        let img = image::ImageReader::open(&path)?
            .with_guessed_format()?
            .decode()?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let decoded_frame = Arc::new(Frame::new(width as usize, height as usize, rgba.to_vec()));
        Ok(Session {
            workflow: WorkFlow::default(),
            decoded: decoded_frame,
        })
    }

    pub fn gen_render_list(&self) -> anyhow::Result<RenderList> {
        self.workflow.compile()
    }

    pub fn get_decoded(&self) -> Arc<Frame> {
        self.decoded.clone()
    }

    /// Return whether need to trigger rerender
    pub fn update(&mut self, diff: WorkFlowDiff) -> bool {
        self.workflow.apply_diff(diff)
    }

    pub fn get_workflow(&self) -> WorkFlow {
        //TODO: Optimize this clone
        self.workflow.clone()
    }
}

pub type NodeId = u64;

/// The editable grade workflow, owned by the runtime's [`EditSession`].
#[derive(Clone, Debug)]
pub struct WorkFlow {
    pub version: u64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    // Two special edge, only record which nodes it connect to
    pub input_to: Vec<NodeId>,
    pub output_from: NodeId,
}

pub struct NodeIdAlloc {
    /// Next available node id
    counter: NodeId,
}

impl NodeIdAlloc {
    fn new() -> Self {
        Self { counter: 0 }
    }
    fn alloc(&mut self) -> NodeId {
        self.counter += 1;
        self.counter - 1
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub node_id: NodeId,
    pub attr: NodeAttr,
    pub pos: (f32, f32),
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
}

#[derive(Clone, Debug)]
pub enum NodeAttr {
    Normal { brightness: f32, saturation: f32 },
    Merge { blend_mode: BlendMode },
}

impl NodeAttr {
    fn default_normal() -> Self {
        Self::Normal {
            brightness: 1.0,
            saturation: 1.0,
        }
    }
    fn default_merge() -> Self {
        Self::Merge {
            blend_mode: BlendMode::Top,
        }
    }
    fn apply_diff(&mut self, diff: NodeAttrDiff) {
        match diff {
            NodeAttrDiff::Brightness(new) => {
                let NodeAttr::Normal {
                    brightness,
                    saturation: _,
                } = self
                else {
                    panic!("Received unmatched node attr diff");
                };
                *brightness = new;
            }
            NodeAttrDiff::Saturation(new) => {
                let NodeAttr::Normal {
                    brightness: _,
                    saturation,
                } = self
                else {
                    panic!("Received unmatched node attr diff");
                };
                *saturation = new;
            }
            NodeAttrDiff::BlendMode(new) => {
                let NodeAttr::Merge { blend_mode } = self else {
                    panic!("Received unmatched node attr diff");
                };
                *blend_mode = new;
            }
        }
    }
}

impl Default for WorkFlow {
    fn default() -> Self {
        let mut allocator = NodeIdAlloc::new();
        let node_id = allocator.alloc();
        WorkFlow {
            version: 0,
            nodes: vec![Node {
                node_id,
                attr: NodeAttr::default_normal(),
                pos: (1.0, 1.0), // TODO: Calculate more accurate default position
            }],
            edges: vec![],
            input_to: vec![node_id],
            output_from: node_id,
        }
    }
}

impl WorkFlow {
    fn compile(&self) -> anyhow::Result<RenderList> {
        Compiler::new().compile(self)
    }

    /// Return whether need to rerender
    fn apply_diff(&mut self, diff: WorkFlowDiff) -> bool {
        if diff.version != self.version {
            info!("Received outdated workflow diff, ignore it");
            return false;
        }
        match diff.payload {
            WorkFlowDiffPayload::NodeAttr(node_id, attr_diff) => {
                self.nodes
                    .iter_mut()
                    .find(|node| node.node_id == node_id)
                    .unwrap()
                    .attr
                    .apply_diff(attr_diff);
            }
            WorkFlowDiffPayload::NodePos(node_id, new_pos) => {
                self.nodes
                    .iter_mut()
                    .find(|node| node.node_id == node_id)
                    .unwrap()
                    .pos = new_pos
            }
        }
        self.version += 1;
        true
    }
}

/// Store a diff for incremental update
/// Only support update one node in a time
#[derive(Clone)]
pub struct WorkFlowDiff {
    pub version: u64,
    pub payload: WorkFlowDiffPayload,
}

#[derive(Clone)]
pub enum WorkFlowDiffPayload {
    NodeAttr(NodeId, NodeAttrDiff),
    NodePos(NodeId, (f32, f32)),
}

#[derive(Clone)]
pub enum NodeAttrDiff {
    Brightness(f32),
    Saturation(f32),
    BlendMode(BlendMode),
}
