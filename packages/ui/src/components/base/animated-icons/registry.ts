import Add from '@hugeicons/core-free-icons/Add01Icon'
import Award from '@hugeicons/core-free-icons/Award01Icon'
import Backpack from '@hugeicons/core-free-icons/Backpack01Icon'
import Bed from '@hugeicons/core-free-icons/BedDoubleIcon'
import Book from '@hugeicons/core-free-icons/BookOpen01Icon'
import Building from '@hugeicons/core-free-icons/Building02Icon'
import Lightbulb from '@hugeicons/core-free-icons/BulbIcon'
import Camera from '@hugeicons/core-free-icons/Camera01Icon'
import Castle from '@hugeicons/core-free-icons/Castle01Icon'
import Chart from '@hugeicons/core-free-icons/ChartColumnIcon'
import Cloud from '@hugeicons/core-free-icons/CloudIcon'
import Coffee from '@hugeicons/core-free-icons/Coffee02Icon'
import Compass from '@hugeicons/core-free-icons/CompassIcon'
import Cpu from '@hugeicons/core-free-icons/CpuIcon'
import Crown from '@hugeicons/core-free-icons/CrownIcon'
import Cube from '@hugeicons/core-free-icons/CubeIcon'
import Database from '@hugeicons/core-free-icons/Database01Icon'
import Dice from '@hugeicons/core-free-icons/DiceIcon'
import Feather from '@hugeicons/core-free-icons/FeatherIcon'
import Film from '@hugeicons/core-free-icons/Film01Icon'
import Flag from '@hugeicons/core-free-icons/Flag01Icon'
import Zap from '@hugeicons/core-free-icons/FlashIcon'
import Flower from '@hugeicons/core-free-icons/FlowerIcon'
import Footprints from '@hugeicons/core-free-icons/FootprintsIcon'
import Gamepad from '@hugeicons/core-free-icons/GameController01Icon'
import Globe from '@hugeicons/core-free-icons/Globe02Icon'
import Headphones from '@hugeicons/core-free-icons/HeadphonesIcon'
import HeartCrack from '@hugeicons/core-free-icons/HeartCrackIcon'
import HeartPulse from '@hugeicons/core-free-icons/HeartPulseIcon'
import Hierarchy from '@hugeicons/core-free-icons/HierarchyIcon'
import Home from '@hugeicons/core-free-icons/Home01Icon'
import Image from '@hugeicons/core-free-icons/Image01Icon'
import Language from '@hugeicons/core-free-icons/LanguageSkillIcon'
import Layout from '@hugeicons/core-free-icons/Layout01Icon'
import Magic from '@hugeicons/core-free-icons/MagicWand01Icon'
import Money from '@hugeicons/core-free-icons/Money01Icon'
import Moon from '@hugeicons/core-free-icons/Moon02Icon'
import Palette from '@hugeicons/core-free-icons/PaintBoardIcon'
import Paintbrush from '@hugeicons/core-free-icons/PaintBrush01Icon'
import Paw from '@hugeicons/core-free-icons/PawPrintIcon'
import Pickaxe from '@hugeicons/core-free-icons/PickaxeIcon'
import Pizza from '@hugeicons/core-free-icons/Pizza01Icon'
import Play from '@hugeicons/core-free-icons/PlayIcon'
import Refresh from '@hugeicons/core-free-icons/RefreshIcon'
import Server from '@hugeicons/core-free-icons/ServerStack01Icon'
import Settings from '@hugeicons/core-free-icons/Settings02Icon'
import Shield from '@hugeicons/core-free-icons/Shield01Icon'
import Skull from '@hugeicons/core-free-icons/SkullIcon'
import Sliders from '@hugeicons/core-free-icons/SlidersHorizontalIcon'
import Lock from '@hugeicons/core-free-icons/SquareLock01Icon'
import Sun from '@hugeicons/core-free-icons/Sun01Icon'
import Swords from '@hugeicons/core-free-icons/Sword03Icon'
import Target from '@hugeicons/core-free-icons/Target01Icon'
import Task from '@hugeicons/core-free-icons/Task01Icon'
import Terminal from '@hugeicons/core-free-icons/TerminalIcon'
import Font from '@hugeicons/core-free-icons/TextFontIcon'
import Toggle from '@hugeicons/core-free-icons/ToggleOnIcon'
import Tree from '@hugeicons/core-free-icons/Tree01Icon'
import Truck from '@hugeicons/core-free-icons/TruckIcon'
import Shirt from '@hugeicons/core-free-icons/TShirtIcon'
import Users from '@hugeicons/core-free-icons/UserGroupIcon'
import Wrench from '@hugeicons/core-free-icons/Wrench01Icon'

// Import individual modules so the launcher bundles only the icons it uses.
export const hugeIcons = {
	award: Award,
	bed: Bed,
	building: Building,
	castle: Castle,
	flag: Flag,
	footprints: Footprints,
	headphones: Headphones,
	'heart-crack': HeartCrack,
	'heart-pulse': HeartPulse,
	home: Home,
	paw: Paw,
	pickaxe: Pickaxe,
	lock: Lock,
	terminal: Terminal,
	font: Font,
	truck: Truck,
	add: Add,
	backpack: Backpack,
	book: Book,
	camera: Camera,
	chart: Chart,
	cloud: Cloud,
	coffee: Coffee,
	compass: Compass,
	cpu: Cpu,
	crown: Crown,
	cube: Cube,
	database: Database,
	dice: Dice,
	feather: Feather,
	film: Film,
	flower: Flower,
	gamepad: Gamepad,
	globe: Globe,
	hierarchy: Hierarchy,
	image: Image,
	language: Language,
	layout: Layout,
	lightbulb: Lightbulb,
	magic: Magic,
	money: Money,
	moon: Moon,
	paintbrush: Paintbrush,
	palette: Palette,
	pizza: Pizza,
	play: Play,
	refresh: Refresh,
	server: Server,
	settings: Settings,
	shield: Shield,
	shirt: Shirt,
	skull: Skull,
	sliders: Sliders,
	sun: Sun,
	swords: Swords,
	target: Target,
	task: Task,
	toggle: Toggle,
	tree: Tree,
	users: Users,
	wrench: Wrench,
	zap: Zap,
} as const

export type AnimatedIconName = keyof typeof hugeIcons

export { partMotion } from '@orbiont/assets/icon-motion'

export const categoryHugeIcons: Record<string, AnimatedIconName> = {
	adventure: 'compass',
	atmosphere: 'cloud',
	audio: 'headphones',
	backpack: 'backpack',
	badge: 'shield',
	'badge-check': 'shield',
	'bed-double': 'bed',
	blocks: 'cube',
	bloom: 'sun',
	'building-2': 'building',
	camera: 'camera',
	cartoon: 'palette',
	castle: 'castle',
	challenging: 'chart',
	clapperboard: 'film',
	cloud: 'cloud',
	'colored-lighting': 'lightbulb',
	combat: 'swords',
	compass: 'compass',
	'core-shaders': 'cpu',
	crown: 'crown',
	cursed: 'skull',
	decoration: 'palette',
	dices: 'dice',
	economy: 'money',
	entities: 'users',
	environment: 'globe',
	equipment: 'swords',
	fantasy: 'magic',
	film: 'film',
	flag: 'flag',
	foliage: 'tree',
	fonts: 'font',
	food: 'pizza',
	footprints: 'footprints',
	'game-mechanics': 'settings',
	'gamepad-2': 'gamepad',
	gauge: 'chart',
	globe: 'globe',
	'grid-3x3': 'layout',
	gui: 'layout',
	handshake: 'users',
	'heart-crack': 'heart-crack',
	'heart-pulse': 'heart-pulse',
	high: 'chart',
	house: 'home',
	items: 'backpack',
	'kitchen-sink': 'cube',
	library: 'book',
	lightweight: 'feather',
	locale: 'language',
	lock: 'lock',
	low: 'chart',
	magic: 'magic',
	management: 'server',
	'map-pinned': 'compass',
	medium: 'chart',
	minigame: 'gamepad',
	mobs: 'users',
	modded: 'wrench',
	models: 'cube',
	multiplayer: 'users',
	network: 'hierarchy',
	optimization: 'zap',
	palette: 'palette',
	'path-tracing': 'sun',
	'paw-print': 'paw',
	pbr: 'cube',
	pickaxe: 'pickaxe',
	potato: 'pizza',
	quests: 'hierarchy',
	realistic: 'image',
	reflections: 'image',
	'refresh-ccw': 'refresh',
	screenshot: 'image',
	'scroll-text': 'task',
	'semi-realistic': 'image',
	shadows: 'moon',
	shield: 'shield',
	simplistic: 'cube',
	skull: 'skull',
	social: 'users',
	square: 'layout',
	storage: 'database',
	sword: 'swords',
	swords: 'swords',
	target: 'target',
	technology: 'server',
	terminal: 'terminal',
	theater: 'film',
	themed: 'palette',
	transportation: 'truck',
	'tree-pine': 'tree',
	trophy: 'award',
	tweaks: 'sliders',
	users: 'users',
	utility: 'wrench',
	'vanilla-like': 'flower',
	'wand-sparkles': 'magic',
	'wifi-off': 'hierarchy',
	worldgen: 'globe',
	zap: 'zap',
}
